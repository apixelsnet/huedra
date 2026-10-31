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
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("huedra: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Version => println!("{APP_NAME} {VERSION}"),
        Command::Help => println!("{APP_NAME} {VERSION}\n\n{USAGE}"),
        Command::Hdr(hdr) => hdr::run(hdr)?,
        Command::Remote(request) => client::run(&request)?,
        Command::Games => games::run()?,
        Command::Tray => app::run()?,
        Command::Settings(page) => {
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
    Ok(())
}

#[cfg(not(windows))]
fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Version => println!("{APP_NAME} {VERSION}"),
        Command::Help => println!("{APP_NAME} {VERSION}\n\n{USAGE}"),
        _ => anyhow::bail!("Huedra runs on Windows only"),
    }
    Ok(())
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
