//! Huedra: one executable with three roles. Without arguments it runs in the
//! notification area; `settings` opens the settings window; everything else
//! is a one-shot command. Feature doc: `docs/features/09-cli-ipc.md`.

#![forbid(unsafe_code)]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod client;
#[cfg(windows)]
mod games;
#[cfg(windows)]
mod hdr;
#[cfg(windows)]
mod launch;
#[cfg(windows)]
mod update;

use std::process::ExitCode;

use huedra_core::command::{self, Command, USAGE};
use huedra_core::{APP_NAME, VERSION};

fn main() -> ExitCode {
    let command = match command::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("huedra: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("huedra: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn run(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        // The started program's exit code is passed on.
        Command::Launch(program) => return launch::run(&program),
        Command::Uninstall => launch::uninstall()?,
        Command::Update(update) => update::run(&update)?,
        Command::Version => println!("{APP_NAME} {VERSION}"),
        Command::Help => println!("{APP_NAME} {VERSION}\n\n{USAGE}"),
        Command::Hdr(hdr) => hdr::run(hdr)?,
        Command::Remote(request) => client::run(&request)?,
        Command::Games => games::run()?,
        Command::Tray => {
            log_panics();
            app::run()?;
        }
        Command::Settings(page) => {
            log_panics();
            let page = match page.as_deref() {
                None => huedra_ui::Page::default(),
                Some(name) => huedra_ui::Page::from_name(name).ok_or_else(|| {
                    anyhow::anyhow!("unknown page `{name}`; use status, games, displays or general")
                })?,
            };
            huedra_win::console::release_own_console();
            huedra_ui::run(huedra_core::locations::Locations::resolve()?, page)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Writes a panic to the log before the process ends. Release builds abort
/// on panic and the tray and the settings window have no console, so this
/// is the only trace. The tray's recovery record still restores SDR on the
/// next start.
#[cfg(windows)]
fn log_panics() {
    use huedra_core::logfile::{self, LogFile};

    let Ok(locations) = huedra_core::locations::Locations::resolve() else {
        return;
    };
    let log = LogFile::new(locations.log, logfile::MAX_BYTES);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let now = huedra_win::clock::now();
        let _ = log.append(&format!(
            "{now} Huedra stopped after an internal error: {info}"
        ));
        previous(info);
    }));
}

#[cfg(not(windows))]
fn run(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Version => println!("{APP_NAME} {VERSION}"),
        Command::Help => println!("{APP_NAME} {VERSION}\n\n{USAGE}"),
        _ => anyhow::bail!("Huedra runs on Windows only"),
    }
    Ok(ExitCode::SUCCESS)
}

/// Opens the settings window as its own process.
#[cfg(windows)]
pub fn open_settings() {
    use std::os::windows::process::CommandExt as _;
    /// `CREATE_NO_WINDOW`: no console window for the child process.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .arg("settings")
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}
