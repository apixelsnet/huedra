//! HDR mods in the settings window: the RenoDX and Luma lists, kept for a
//! day, and installing or removing a mod (also one installed by hand) on a
//! background thread, and once a day whether an installed mod has a newer
//! version. What is checked, and the record that makes every install
//! reversible, are in `huedra_core::mods`.
//! Feature doc: `docs/features/16-hdr-mods.md`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use huedra_core::library::GameId;
use huedra_core::mods::{self, Arch, Freshness, Manifest, Mod, Source, Validators};
use huedra_win::{hash, http, process};

const RENODX_FILE: &str = "renodx.md";
const LUMA_FILE: &str = "luma.md";
/// Longest an unpacking may take.
const TAR_TIMEOUT: Duration = Duration::from_secs(120);

fn user_agent() -> String {
    format!(
        "Huedra/{} (Windows HDR switching; reads the RenoDX and Luma lists of HDR mods)",
        huedra_core::VERSION
    )
}

/// The lists as last downloaded; empty before the first download.
pub fn load_lists(folder: &Path) -> Vec<Mod> {
    let read = |name: &str| fs::read_to_string(folder.join(name)).unwrap_or_default();
    let mut found = mods::parse_renodx(&read(RENODX_FILE));
    found.extend(mods::parse_luma(&read(LUMA_FILE)));
    found
}

fn fresh(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|time| SystemTime::now().duration_since(time).ok())
        .is_some_and(|age| age.as_secs() < mods::LIST_MAX_AGE_SECS)
}

/// Downloads the lists on a background thread when they are older than a
/// day, then hands the new lists to `deliver` (on that thread).
pub fn refresh_lists(folder: PathBuf, deliver: impl FnOnce(Vec<Mod>) + Send + 'static) {
    let _ = thread::Builder::new()
        .name("huedra-mod-lists".into())
        .spawn(move || {
            let mut changed = false;
            for (name, path) in [
                (RENODX_FILE, mods::RENODX_LIST),
                (LUMA_FILE, mods::LUMA_LIST),
            ] {
                let file = folder.join(name);
                if fresh(&file) {
                    continue;
                }
                let Ok(response) =
                    http::get(mods::LIST_HOST, path, &user_agent(), mods::MAX_LIST_BYTES)
                else {
                    continue;
                };
                if response.status == 200
                    && fs::create_dir_all(&folder).is_ok()
                    && huedra_core::files::write_atomic(&file, &response.body).is_ok()
                {
                    changed = true;
                }
            }
            if changed {
                deliver(load_lists(&folder));
            }
        });
}

/// The program a mod goes next to: of the programs Huedra saw the game run,
/// the one that is still there and largest (a launcher is small, the game
/// itself large).
pub fn program_of(programs: &[String]) -> Option<PathBuf> {
    programs
        .iter()
        .map(PathBuf::from)
        .filter_map(|p| fs::metadata(&p).ok().map(|m| (m.len(), p)))
        .max_by_key(|(len, _)| *len)
        .map(|(_, p)| p)
}

/// What an install will do, worked out before the user is asked.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The game.
    pub game: GameId,
    /// The mod.
    pub entry: Mod,
    /// The download for the game's architecture.
    pub url: String,
    /// The game's architecture.
    pub arch: Arch,
    /// The folder the files go into (the program's).
    pub folder: PathBuf,
    /// ReShade's name in the game folder (RenoDX only).
    pub proxy: Option<&'static str>,
    /// The add-on's file name (RenoDX only).
    pub addon: Option<String>,
    /// The installed version this install replaces (an update).
    pub replaces: Option<Manifest>,
}

impl Plan {
    /// What will change, for the confirmation dialog.
    pub fn changes(&self) -> Vec<String> {
        let host = mods::trusted(&self.url).map_or("", |(host, _)| host);
        let mut lines = Vec::new();
        if self.replaces.is_some() {
            lines.push(
                "Take out the installed version and put back the game's own files".to_owned(),
            );
        }
        match (self.entry.source, self.proxy, &self.addon) {
            (Source::RenoDx, Some(proxy), Some(addon)) => {
                lines.push(format!(
                    "Download ReShade from reshade.me and add it as {proxy}"
                ));
                lines.push(format!("Download {addon} from {host} and add it"));
            }
            _ => lines.push(format!(
                "Download the mod from {host} and add its files: its own ReShade (dxgi.dll), its add-on and the Luma folder"
            )),
        }
        lines.push(format!("Into {}", self.folder.display()));
        lines.push(
            "Files it replaces are kept in Huedra's folder and come back when you remove the mod"
                .to_owned(),
        );
        lines.push(
            "Windows Auto HDR and NVIDIA RTX HDR are turned off for this game while it is installed"
                .to_owned(),
        );
        lines
    }
}

/// Works out an install for a game's program, or says why it cannot be done.
///
/// # Errors
/// A sentence for the user.
pub fn plan(game: &GameId, entry: &Mod, program: &Path) -> Result<Plan, String> {
    let folder = program
        .parent()
        .ok_or("The game's folder is unknown.")?
        .to_path_buf();
    if folder
        .to_string_lossy()
        .to_lowercase()
        .contains(r"\windowsapps\")
    {
        return Err(
            "Games from the Microsoft Store are in a protected folder that mods can't be added to."
                .into(),
        );
    }
    let read = fs::File::open(program)
        .ok()
        .and_then(|mut file| mods::read_program(&mut file).ok().flatten())
        .ok_or_else(|| format!("Huedra couldn't read {}.", program.display()))?;
    let url = entry
        .url(read.arch)
        .ok_or(match read.arch {
            Arch::X64 => "This mod is for 32-bit games, and this game is 64-bit.",
            Arch::X86 => "This mod is for 64-bit games, and this game is 32-bit.",
        })?
        .to_owned();
    if mods::trusted(&url).is_none() {
        return Err(
            "The mod's download isn't on a site Huedra downloads from (GitHub or reshade.me)."
                .into(),
        );
    }
    let (proxy, addon) = match entry.source {
        Source::RenoDx => {
            let proxy = mods::proxy_dll(&read).ok_or(
                "The game uses Vulkan, which ReShade reaches another way. Install this one by hand.",
            )?;
            let addon = mods::file_name(&url).ok_or("The mod's file name can't be used.")?;
            (Some(proxy), Some(addon))
        }
        Source::Luma => (None, None),
    };
    Ok(Plan {
        game: game.clone(),
        entry: entry.clone(),
        url,
        arch: read.arch,
        folder,
        proxy,
        addon,
        replaces: None,
    })
}

/// Whether a program in the folder (or below it) is running.
fn running_in(folder: &Path) -> bool {
    let mut prefix = folder.to_string_lossy().to_lowercase();
    if !prefix.ends_with('\\') {
        prefix.push('\\');
    }
    process::pids()
        .into_iter()
        .filter_map(process::image_path)
        .any(|path| path.to_lowercase().starts_with(&prefix))
}

fn validators(headers: http::Headers) -> Validators {
    Validators {
        etag: headers.etag,
        modified: headers.modified,
        length: headers.length,
    }
}

/// Downloads a file from a trusted site straight to disk; returns what the
/// server said about it.
fn fetch_to(url: &str, file: &Path) -> Result<Validators, String> {
    let (host, path) = mods::trusted(url).ok_or("The address isn't on a trusted site.")?;
    match http::download(host, path, &user_agent(), mods::MAX_DOWNLOAD_BYTES, file) {
        // Refused before anything is unpacked or copied; the work folder
        // with the file goes with the failed install.
        Ok((_, headers)) if !mods::trusted_final(&headers.final_url) => Err(format!(
            "The download from {host} was sent on to {}, which isn't a site Huedra downloads mods from.",
            site_of(&headers.final_url)
        )),
        Ok((200, headers)) => Ok(validators(headers)),
        Ok((status, _)) => Err(format!("The download from {host} failed (HTTP {status}).")),
        Err(http::HttpError::TooLarge(max)) => Err(format!(
            "The download from {host} is larger than Huedra accepts ({} MB).",
            max >> 20
        )),
        Err(http::HttpError::File(error)) => {
            Err(format!("Huedra couldn't save the download ({error})."))
        }
        Err(_) => Err(format!(
            "The download from {host} failed. Check the connection and try again."
        )),
    }
}

/// The host of an address, for messages.
fn site_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    match rest.split(['/', '?', '#']).next() {
        Some(host) if !host.is_empty() => host,
        _ => "an unknown address",
    }
}

/// The newest ReShade setup with add-on support, by the versioned link on
/// ReShade's home page (the short link redirects through plain http).
fn reshade_setup_to(file: &Path) -> Result<(), String> {
    let newest = http::get("reshade.me", "/", &user_agent(), mods::MAX_LIST_BYTES)
        .ok()
        .filter(|page| page.status == 200)
        .and_then(|page| mods::reshade_setup_url(&String::from_utf8_lossy(&page.body)));
    fetch_to(newest.as_deref().unwrap_or(mods::RESHADE_URL), file).map(drop)
}

/// The SHA-256 of a downloaded file when it is small enough to compare
/// later; empty otherwise.
fn download_hash(file: &Path) -> String {
    let small = fs::metadata(file)
        .is_ok_and(|m| usize::try_from(m.len()).is_ok_and(|len| len <= mods::COMPARE_MAX_BYTES));
    if !small {
        return String::new();
    }
    fs::read(file)
        .map(|data| hash::sha256_hex(&data))
        .unwrap_or_default()
}

fn unpack(tar: &Path, archive: &Path, into: &Path, only: Option<&str>) -> Result<(), String> {
    let mut args: Vec<&OsStr> = vec![
        OsStr::new("-xf"),
        archive.as_os_str(),
        OsStr::new("-C"),
        into.as_os_str(),
    ];
    if let Some(only) = only {
        args.push(OsStr::new(only));
    }
    if process::run_hidden(tar, &args, TAR_TIMEOUT) {
        Ok(())
    } else {
        Err("The download couldn't be unpacked.".into())
    }
}

const CANCELED: &str = "Canceled. Nothing in the game folder was changed.";

fn run_install(
    plan: &Plan,
    mods_dir: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(&str, f32),
) -> Result<Manifest, String> {
    if running_in(&plan.folder) {
        return Err("Close the game first, then install the mod.".into());
    }
    // An update stages beside the work folder: taking out the old version
    // deletes the work folder.
    let work = if plan.replaces.is_some() {
        update_folder(mods_dir, &plan.game)
    } else {
        mods::work_folder(mods_dir, &plan.game)
    };
    let download = work.join("download");
    let staged = work.join("staged");
    for dir in [&download, &staged] {
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).map_err(|e| format!("Huedra couldn't make a folder ({e})."))?;
    }
    let tar = process::system_folder()
        .map(|dir| dir.join("tar.exe"))
        .filter(|p| p.is_file())
        .ok_or("Windows' tar.exe, which unpacks the download, is missing.")?;
    let canceled = || cancel.load(Ordering::Relaxed);
    let served: Validators;
    let sha256: String;
    let files: Vec<(String, String)> = match (plan.entry.source, plan.proxy, &plan.addon) {
        (Source::RenoDx, Some(proxy), Some(addon)) => {
            progress("Downloading ReShade from reshade.me", 0.1);
            let setup = download.join("reshade-setup.exe");
            reshade_setup_to(&setup)?;
            if canceled() {
                return Err(CANCELED.into());
            }
            progress("Unpacking ReShade", 0.35);
            let dll = match plan.arch {
                Arch::X64 => "ReShade64.dll",
                Arch::X86 => "ReShade32.dll",
            };
            unpack(&tar, &setup, &staged, Some(dll))?;
            if !staged.join(dll).is_file() {
                return Err("ReShade wasn't in the download.".into());
            }
            if canceled() {
                return Err(CANCELED.into());
            }
            progress(&format!("Downloading {addon}"), 0.55);
            served = fetch_to(&plan.url, &staged.join(addon))?;
            sha256 = download_hash(&staged.join(addon));
            vec![
                (dll.to_owned(), proxy.to_owned()),
                (addon.clone(), addon.clone()),
            ]
        }
        _ => {
            progress("Downloading the mod from GitHub", 0.15);
            let archive = download.join("mod.zip");
            served = fetch_to(&plan.url, &archive)?;
            sha256 = download_hash(&archive);
            if canceled() {
                return Err(CANCELED.into());
            }
            progress("Unpacking the mod", 0.5);
            unpack(&tar, &archive, &staged, None)?;
            let found =
                mods::files_in(&staged).map_err(|e| format!("The mod can't be used ({e})."))?;
            if found.is_empty() {
                return Err("The download held no files.".into());
            }
            found.into_iter().map(|f| (f.clone(), f)).collect()
        }
    };
    if canceled() {
        return Err(CANCELED.into());
    }
    // ReShade's settings files that appeared with the old version (the
    // mod's settings): kept through an update and still taken out by a
    // later Remove.
    let mut inherited: Vec<String> = Vec::new();
    if let Some(old) = &plan.replaces {
        progress("Taking out the installed version", 0.7);
        let changed = mods::changed_files(old, &hash::sha256_hex)
            .map_err(|e| format!("Huedra couldn't read the installed version ({e})."))?;
        if !changed.is_empty() {
            return Err(format!(
                "These files changed after the install (a game update or another mod): {}. Remove the mod, then install it again.",
                changed.join(", ")
            ));
        }
        let mut old = old.clone();
        if Path::new(&old.folder) == plan.folder {
            inherited = std::mem::take(&mut old.absent);
        }
        let kept = mods::remove(mods_dir, &old, &hash::sha256_hex).map_err(|e| {
            format!(
                "Taking out the installed version stopped ({e}). Close the game and use Remove."
            )
        })?;
        if !kept.is_empty() {
            return Err(
                "Some files of the installed version changed while it was taken out. Use Remove."
                    .into(),
            );
        }
    }
    progress("Copying the files into the game folder", 0.8);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut manifest = Manifest {
        game: plan.game.clone(),
        name: plan.entry.name.clone(),
        source: plan.entry.source,
        author: plan.entry.author.clone(),
        url: plan.url.clone(),
        folder: plan.folder.display().to_string(),
        installed: now,
        complete: false,
        files: Vec::new(),
        dirs: Vec::new(),
        absent: Vec::new(),
        etag: String::new(),
        modified: String::new(),
        length: 0,
        download_sha256: sha256,
        checked: now,
        newer: false,
    };
    manifest.record(&served);
    let result = mods::place(
        mods_dir,
        &mut manifest,
        &staged,
        &files,
        &hash::sha256_hex,
        &canceled,
    );
    let _ = fs::remove_dir_all(&download);
    let _ = fs::remove_dir_all(&staged);
    match result {
        Ok(()) if inherited.is_empty() => Ok(manifest),
        Ok(()) => {
            for name in inherited {
                if !manifest.absent.contains(&name) {
                    manifest.absent.push(name);
                }
            }
            // The install is complete either way; a failed save only means
            // Remove leaves ReShade's settings files behind.
            let _ = manifest.save(&mods::manifest_path(mods_dir, &manifest.game));
            Ok(manifest)
        }
        Err(error) => {
            let undone = mods::remove(mods_dir, &manifest, &hash::sha256_hex).is_ok();
            Err(match (error.kind(), undone) {
                (std::io::ErrorKind::Interrupted, true) => CANCELED.into(),
                (_, true) => format!(
                    "Copying into the game folder failed ({error}). What was copied has been taken out again."
                ),
                (_, false) => format!(
                    "Copying into the game folder failed ({error}). Use Remove to take out what was copied."
                ),
            })
        }
    }
}

/// Installs a mod on a background thread. `progress` gets each step and a
/// fraction of the way; `done` gets the record or why it failed. Setting
/// `cancel` stops it and takes back what was copied.
pub fn install(
    plan: Plan,
    mods_dir: PathBuf,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(&str, f32) + Send + 'static,
    done: impl FnOnce(Result<Manifest, String>) + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("huedra-mod-install".into())
        .spawn(move || {
            let result = run_install(&plan, &mods_dir, &cancel, &progress);
            if result.is_err() {
                // The downloads go; the folder too when nothing else is in it
                // (a backup kept by a removal that left files stays).
                let work = mods::work_folder(&mods_dir, &plan.game);
                let _ = fs::remove_dir_all(work.join("download"));
                let _ = fs::remove_dir_all(work.join("staged"));
            }
            let _ = fs::remove_dir_all(update_folder(&mods_dir, &plan.game));
            // An empty work folder goes either way.
            let _ = fs::remove_dir(mods::work_folder(&mods_dir, &plan.game));
            done(result);
        });
}

/// Where an update downloads and unpacks the new version.
fn update_folder(mods_dir: &Path, game: &GameId) -> PathBuf {
    mods_dir.join(format!("{}.update", mods::key(game)))
}

/// Checks the installed mods whose check is due for a newer version, on a
/// background thread, saving what it learns in their records; `done` gets
/// the records that changed. A failed check waits for the next day.
pub fn check_updates(
    records: Vec<Manifest>,
    lists: Vec<Mod>,
    mods_dir: PathBuf,
    done: impl FnOnce(Vec<Manifest>) + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("huedra-mod-updates".into())
        .spawn(move || {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let mut changed = Vec::new();
            for mut record in records.into_iter().filter(|r| r.check_due(now)) {
                record.checked = now;
                let entry = mods::find(&lists, &record.name).filter(|m| m.source == record.source);
                if entry.is_some_and(|e| mods::link_changed(&record, e)) {
                    record.newer = true;
                } else if let Some(served) = probe(&record.url) {
                    match mods::freshness(&record.validators(), &served) {
                        Freshness::Same | Freshness::Unknown => {}
                        Freshness::Newer => record.newer = true,
                        Freshness::Compare => {
                            if let Some(sha256) = fetch_hash(&record.url) {
                                match record.matches_install(&sha256) {
                                    Some(same) => record.newer = !same,
                                    // An old record: this download is the
                                    // baseline from now on.
                                    None => record.download_sha256 = sha256,
                                }
                            }
                        }
                    }
                    if !record.newer {
                        record.record(&served);
                    }
                }
                let _ = record.save(&mods::manifest_path(&mods_dir, &record.game));
                changed.push(record);
            }
            done(changed);
        });
}

/// What the server says about a download now, without downloading it.
fn probe(url: &str) -> Option<Validators> {
    let (host, path) = mods::trusted(url)?;
    match http::probe(host, path, &user_agent()) {
        Ok((200 | 206, headers)) => Some(validators(headers)),
        _ => None,
    }
}

/// Downloads a small file into memory and returns its SHA-256.
fn fetch_hash(url: &str) -> Option<String> {
    let (host, path) = mods::trusted(url)?;
    let response = http::get(host, path, &user_agent(), mods::COMPARE_MAX_BYTES).ok()?;
    (response.status == 200).then(|| hash::sha256_hex(&response.body))
}

/// Removes a mod on a background thread; `done` gets the files left in
/// place because they changed since, or why it stopped.
pub fn uninstall(
    manifest: Manifest,
    mods_dir: PathBuf,
    done: impl FnOnce(Result<Vec<String>, String>) + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("huedra-mod-remove".into())
        .spawn(move || {
            let result = if running_in(Path::new(&manifest.folder)) {
                Err("Close the game first, then remove the mod.".to_owned())
            } else {
                mods::remove(&mods_dir, &manifest, &hash::sha256_hex).map_err(|e| {
                    format!("Removing stopped ({e}). Close the game and try again; Huedra continues where it stopped.")
                })
            };
            done(result);
        });
}

/// Works out on a background thread what removing the mod installed by hand
/// in `folder` takes out (reading DLLs to find ReShade takes a moment).
pub fn plan_by_hand(
    folder: PathBuf,
    done: impl FnOnce(Result<mods::HandRemoval, String>) + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("huedra-mod-look".into())
        .spawn(move || {
            done(
                mods::plan_hand_removal(&folder)
                    .map_err(|e| format!("Huedra couldn't read {} ({e}).", folder.display())),
            );
        });
}

/// Removes a mod installed by hand on a background thread, keeping a copy
/// of every file in `keep`; `done` gets why it stopped, if it did.
pub fn uninstall_by_hand(
    removal: mods::HandRemoval,
    keep: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("huedra-mod-remove".into())
        .spawn(move || {
            let result = if running_in(&removal.folder) {
                Err("Close the game first, then remove the mod.".to_owned())
            } else {
                mods::remove_by_hand(&removal, &keep).map_err(|e| {
                    format!("Removing stopped ({e}). Close the game and try again; Huedra continues with what is left.")
                })
            };
            done(result);
        });
}
