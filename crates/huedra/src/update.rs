//! `huedra update`: checks for a newer signed release, installs it next to
//! the running program, and rolls back to the previous one.
//! Feature doc: `docs/features/11-updates.md`.
//!
//! Order of trust: the manifest's signature is checked first; the program is
//! accepted only when its size and SHA-256 match the signed manifest and it
//! reports the signed version. Only then is the tray closed and the files
//! swapped by renaming, which Windows allows for a running program.

use std::fs;
use std::io::Write as _;
use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, bail};
use huedra_core::APP_NAME;
use huedra_core::command::{BACKGROUND, UpdateCommand};
use huedra_core::config::{Config, UpdatePolicy};
use huedra_core::files;
use huedra_core::ipc::{Request, Response};
use huedra_core::locations::Locations;
use huedra_core::logfile::{self, LogFile};
use huedra_core::update::{self, Release, ReleaseNotes, ReleaseUrl, UpdateState, Verdict, Version};
use huedra_win::pipe::{self, PipeError};
use huedra_win::{clock, hash, http, process};

const USER_AGENT: &str = concat!("Huedra/", env!("CARGO_PKG_VERSION"));
/// How long the closing tray may take to give up its channel.
const TRAY_CLOSE_WAIT: Duration = Duration::from_secs(10);
/// How often, and how far apart, the program an update moved aside is
/// tried for removal: the process that ran the update still holds it for
/// a moment after the new version starts.
const REMOVE_ATTEMPTS: u32 = 60;
const REMOVE_INTERVAL: Duration = Duration::from_millis(500);
/// `CREATE_NO_WINDOW`: no console window for the version check.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Runs one form of `huedra update`.
pub fn run(command: &UpdateCommand) -> anyhow::Result<()> {
    let locations = Locations::resolve()?;
    let updater = Updater {
        log: LogFile::new(locations.log.clone(), logfile::MAX_BYTES),
        locations,
    };
    let result = match command {
        UpdateCommand::Run { check_only, from } => updater.run_now(*check_only, from.as_deref()),
        UpdateCommand::Auto => {
            huedra_win::console::release_own_console();
            if let Err(error) = updater.auto() {
                updater.log(&format!("The update check failed: {error:#}"));
            }
            return Ok(());
        }
    };
    // Started from the tray menu there is no console, so the log is the
    // only place the reason can be read.
    if let Err(error) = &result {
        updater.log(&format!("The update failed: {error:#}"));
    }
    result
}

/// Deletes the program an update moved aside (`huedra.previous.exe`), in
/// the background, once the process that ran the update has let go of it.
/// Called when the tray starts; a file still in use after half a minute is
/// left for the next start.
pub fn remove_previous() {
    let Ok(files) = Files::of_running() else {
        return;
    };
    if !files.previous.is_file() {
        return;
    }
    std::thread::spawn(move || {
        for _ in 0..REMOVE_ATTEMPTS {
            if remove_if_present(&files.previous).is_ok() {
                return;
            }
            std::thread::sleep(REMOVE_INTERVAL);
        }
    });
}

struct Updater {
    locations: Locations,
    log: LogFile,
}

impl Updater {
    /// `huedra update [--check] [--from <folder>]`.
    fn run_now(&self, check_only: bool, from: Option<&str>) -> anyhow::Result<()> {
        let key = key()?;
        let source = match from {
            Some(folder) => Source::Folder(PathBuf::from(folder)),
            None => Source::Web(update::release_url().context(
                "this build of Huedra has no release address, so it cannot look for updates",
            )?),
        };
        let release = fetch_release(&source, key)?;
        let running = Version::current();
        let mut state = UpdateState::load(&self.locations.update);
        if matches!(source, Source::Web(_)) {
            state.last_check = update::unix_secs(SystemTime::now());
        }
        let Verdict::Newer(version) = update::verdict(release.version(), running) else {
            state.available = None;
            self.save(&state);
            println!("Huedra {running} is up to date.");
            return Ok(());
        };
        if check_only {
            state.available = Some(version);
            self.save(&state);
            self.keep_notes(&source, key, version);
            println!(
                "Huedra {version} is available (this is {running}). Install it with: huedra update"
            );
            return Ok(());
        }
        let tray = tray()?;
        if tray == Tray::Busy {
            bail!(
                "a game or video is using HDR; install the update after it ends, so HDR is not interrupted"
            );
        }
        let program = download_program(&source, &release)?;
        let files = Files::of_running()?;
        self.install(&files, &program, version, tray == Tray::Running)?;
        state.available = None;
        self.save(&state);
        self.log(&format!("Huedra was updated from {running} to {version}."));
        println!("Huedra was updated from {running} to {version}.");
        Ok(())
    }

    /// The daily check the tray starts, following the Updates setting.
    fn auto(&self) -> anyhow::Result<()> {
        let config = Config::load(&self.locations.config).unwrap_or_default();
        let policy = config.general.updates;
        let (Some(key), Some(url)) = (update::public_key(), update::release_url()) else {
            return Ok(());
        };
        if policy == UpdatePolicy::Off {
            return Ok(());
        }
        let source = Source::Web(url);
        let release = fetch_release(&source, key)?;
        let running = Version::current();
        let mut state = UpdateState::load(&self.locations.update);
        state.last_check = update::unix_secs(SystemTime::now());
        let version = match update::verdict(release.version(), running) {
            Verdict::UpToDate => {
                state.available = None;
                self.save(&state);
                self.log(&format!(
                    "Checked for updates: Huedra {running} is up to date."
                ));
                return Ok(());
            }
            Verdict::Newer(version) => version,
        };
        if policy == UpdatePolicy::Ask {
            state.available = Some(version);
            self.save(&state);
            self.keep_notes(&source, key, version);
            self.log(&format!(
                "Huedra {version} is available; it waits for your consent."
            ));
            let _ = pipe::call(&Request::UpdateAvailable {
                version: version.to_string(),
            });
            return Ok(());
        }
        let tray = tray()?;
        if tray == Tray::Busy {
            self.save(&state);
            self.log(&format!(
                "Huedra {version} is available; it will be installed when no game or video uses HDR."
            ));
            return Ok(());
        }
        let program = download_program(&source, &release)?;
        let files = Files::of_running()?;
        self.install(&files, &program, version, tray == Tray::Running)?;
        state.available = None;
        self.save(&state);
        self.log(&format!(
            "Huedra was updated automatically from {running} to {version}."
        ));
        Ok(())
    }

    /// Downloads the release notes of `version`, checks their signature and
    /// keeps them for the update dialog. Without them the dialog links to
    /// the changelog instead.
    fn keep_notes(&self, source: &Source, key: &str, version: Version) {
        let path = update::notes_path(&self.locations.update);
        let notes = source
            .fetch(update::NOTES_NAME, update::MAX_NOTES_BYTES)
            .and_then(|notes| {
                let signature =
                    source.fetch(update::NOTES_SIGNATURE_NAME, update::MAX_SIGNATURE_BYTES)?;
                let signature =
                    String::from_utf8(signature).context("the notes' signature is not text")?;
                Ok(ReleaseNotes::verify(&notes, &signature, key, version)?)
            })
            .and_then(|text| {
                files::write_atomic(&path, text.as_bytes())
                    .with_context(|| format!("could not write {}", path.display()))
            });
        if let Err(error) = notes {
            let _ = fs::remove_file(&path);
            self.log(&format!(
                "The release notes of Huedra {version} were not read: {error:#}."
            ));
        }
    }

    /// Writes the new program, checks it starts and reports its version,
    /// closes the tray, swaps the files and starts the tray again.
    fn install(
        &self,
        files: &Files,
        program: &[u8],
        version: Version,
        tray_running: bool,
    ) -> anyhow::Result<()> {
        remove_if_present(&files.new)?;
        write_synced(&files.new, program).with_context(|| {
            format!(
                "could not write to {}; if the folder needs administrator rights, replace huedra.exe by hand",
                files.folder().display()
            )
        })?;
        let ready = check_reports(&files.new, version).and_then(|()| {
            remove_if_present(&files.previous).with_context(|| {
                format!(
                    "could not remove {} (is a window of that version still open?)",
                    files.previous.display()
                )
            })
        });
        if let Err(error) = ready {
            let _ = fs::remove_file(&files.new);
            return Err(error);
        }
        if tray_running {
            stop_tray()?;
        }
        let swapped = fs::rename(&files.current, &files.previous)
            .context("could not move the running program aside")
            .and_then(|()| {
                fs::rename(&files.new, &files.current).or_else(|error| {
                    let _ = fs::rename(&files.previous, &files.current);
                    Err(error).context("could not put the new program in place")
                })
            });
        if tray_running {
            process::spawn_detached(&files.current, &[BACKGROUND]);
        }
        swapped
    }

    fn save(&self, state: &UpdateState) {
        if let Err(error) = state.save(&self.locations.update) {
            self.log(&format!("Could not save the update record: {error}"));
        }
    }

    fn log(&self, message: &str) {
        let _ = self.log.append(&format!("{} {message}", clock::now()));
    }
}

fn key() -> anyhow::Result<&'static str> {
    update::public_key()
        .context("this build of Huedra has no update key, so it cannot check releases")
}

/// Where the release files come from.
enum Source {
    /// The release address.
    Web(ReleaseUrl),
    /// A folder holding downloaded release files.
    Folder(PathBuf),
}

impl Source {
    fn fetch(&self, name: &str, max_bytes: usize) -> anyhow::Result<Vec<u8>> {
        match self {
            Self::Web(url) => {
                let response = http::get(url.host(), &url.path_of(name), USER_AGENT, max_bytes)
                    .with_context(|| format!("could not download {name}"))?;
                if response.status != 200 {
                    bail!(
                        "could not download {name}: the server answered {}",
                        response.status
                    );
                }
                Ok(response.body)
            }
            Self::Folder(dir) => {
                let path = dir.join(name);
                let len = fs::metadata(&path)
                    .with_context(|| format!("could not read {}", path.display()))?
                    .len();
                if len > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
                    bail!("{} is larger than {max_bytes} bytes", path.display());
                }
                fs::read(&path).with_context(|| format!("could not read {}", path.display()))
            }
        }
    }
}

fn fetch_release(source: &Source, key: &str) -> anyhow::Result<Release> {
    let manifest = source.fetch(update::MANIFEST_NAME, update::MAX_MANIFEST_BYTES)?;
    let signature = source.fetch(update::SIGNATURE_NAME, update::MAX_SIGNATURE_BYTES)?;
    let signature = String::from_utf8(signature).context("the signature file is not text")?;
    Ok(Release::verify(&manifest, &signature, key)?)
}

fn download_program(source: &Source, release: &Release) -> anyhow::Result<Vec<u8>> {
    let max = usize::try_from(update::MAX_PROGRAM_BYTES).unwrap_or(usize::MAX);
    let program = source.fetch(release.file(), max)?;
    let digest = hash::sha256(&program).context("could not compute the SHA-256 digest")?;
    release.check_program(u64::try_from(program.len()).unwrap_or(u64::MAX), &digest)?;
    Ok(program)
}

/// Runs the new program with `--version`, so a release that does not start
/// on this PC is never put in place.
fn check_reports(program: &Path, version: Version) -> anyhow::Result<()> {
    let output = std::process::Command::new(program)
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("the new program did not start")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let expected = format!("{APP_NAME} {version}");
    if !output.status.success() || text.trim() != expected {
        bail!(
            "the new program reports `{}` instead of `{expected}`",
            text.trim()
        );
    }
    Ok(())
}

/// The running program and the files kept beside it.
struct Files {
    current: PathBuf,
    new: PathBuf,
    previous: PathBuf,
}

impl Files {
    fn of_running() -> anyhow::Result<Self> {
        let current = std::env::current_exe().context("could not find the running program")?;
        let stem = current
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .context("the running program has no name")?;
        let beside = |suffix: &str| current.with_file_name(format!("{stem}.{suffix}.exe"));
        Ok(Self {
            new: beside("new"),
            previous: beside("previous"),
            current,
        })
    }

    fn folder(&self) -> &Path {
        self.current.parent().unwrap_or(Path::new("."))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tray {
    NotRunning,
    Running,
    /// Running, and a game, video or launch is using HDR.
    Busy,
}

fn tray() -> anyhow::Result<Tray> {
    match pipe::call(&Request::Status) {
        Ok(Response::Status(report)) if report.busy => Ok(Tray::Busy),
        Ok(_) => Ok(Tray::Running),
        Err(PipeError::NotRunning) => Ok(Tray::NotRunning),
        Err(error) => Err(error).context("could not ask the running Huedra"),
    }
}

fn stop_tray() -> anyhow::Result<()> {
    match pipe::call(&Request::Exit) {
        Ok(_) | Err(PipeError::NotRunning) => {}
        Err(error) => return Err(error).context("could not close the running Huedra"),
    }
    if !pipe::wait_closed(TRAY_CLOSE_WAIT) {
        bail!("the running Huedra did not close");
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("could not remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

fn write_synced(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}
