//! `huedra launch` and `huedra uninstall`.
//! Feature docs: `docs/features/06-launch-with-hdr.md`, `12-packaging.md`.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use huedra_core::ipc::{Request, Response};
use huedra_core::launch::{self, LaunchCommand};
use huedra_core::locations::Locations;
use huedra_win::job::Suspended;
use huedra_win::pipe::{self, PipeError};
use huedra_win::{process, registry};

/// How long to wait for a tray process started by `launch` to answer.
const TRAY_START_WAIT: Duration = Duration::from_secs(15);
const TRAY_POLL: Duration = Duration::from_millis(250);

/// Creates the program suspended, turns HDR on through the tray process,
/// lets the program run, and waits until it and the programs it started
/// from its folder have ended. The tray keeps HDR on while this process
/// lives. The program runs even when HDR could not be turned on; the reason
/// is printed. A program that cannot be started leaves HDR alone (exit 2).
pub fn run(command: &LaunchCommand) -> anyhow::Result<ExitCode> {
    huedra_win::console::release_own_console();
    // Created suspended first: a program that cannot be started is
    // reported before HDR changes for it.
    let suspended = match Suspended::create(&command.program, &command.args) {
        Ok(suspended) => suspended,
        Err(error) => {
            eprintln!("huedra: could not start {}: {error}", command.program);
            return Ok(ExitCode::from(2));
        }
    };
    let program = std::path::absolute(&command.program)
        .map_or_else(|_| command.program.clone(), |p| p.display().to_string());
    let request = Request::Launch {
        pid: std::process::id(),
        program: program.clone(),
    };
    match ask_tray(&request) {
        Ok(Response::Done { message }) => eprintln!("{message}"),
        Ok(Response::Failed { message }) => eprintln!("huedra: {message}"),
        Ok(Response::Status(_)) => {}
        Err(error) => eprintln!("huedra: HDR was not turned on: {error}"),
    }
    let launched = suspended
        .resume()
        .with_context(|| format!("could not start {}", command.program))?;
    // The path Windows started, which differs from the one given when the
    // program was found through PATH.
    let started = launched.path().unwrap_or(program);
    let working_dir = std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let status = launched
        .wait(&launch::wait_folder(&started, &working_dir))
        .context("could not wait for the program")?;
    // Codes outside 0 to 255 cannot be passed on; they become 1.
    Ok(match status.code() {
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => ExitCode::FAILURE,
    })
}

/// Sends the request, starting the tray process first when none runs.
fn ask_tray(request: &Request) -> Result<Response, PipeError> {
    match pipe::call(request) {
        Err(PipeError::NotRunning) => {}
        other => return other,
    }
    start_tray();
    let deadline = Instant::now() + TRAY_START_WAIT;
    loop {
        std::thread::sleep(TRAY_POLL);
        match pipe::call(request) {
            Err(PipeError::NotRunning) if Instant::now() < deadline => {}
            other => return other,
        }
    }
}

/// Starts the tray process outside any job this process is in, so a store
/// that waits for a game's processes (Steam) does not wait for Huedra too.
fn start_tray() {
    if let Ok(exe) = std::env::current_exe() {
        process::spawn_detached(&exe, &[huedra_core::command::BACKGROUND]);
    }
}

/// Closes the running tray process and removes every registry entry
/// Huedra wrote. Files are left for the user to delete.
pub fn uninstall() -> anyhow::Result<()> {
    match pipe::call(&Request::Exit) {
        Ok(_) => println!("Huedra has closed."),
        Err(PipeError::NotRunning) => {}
        Err(error) => anyhow::bail!("could not close the running Huedra: {error}"),
    }
    registry::remove_all().context("could not remove Huedra's registry entries")?;
    // Auto HDR that Huedra set for games' programs goes back to Windows'
    // own setting; the rest of each program's graphics settings stays.
    if let Ok(locations) = Locations::resolve()
        && let Ok(config) = huedra_core::config::Config::load(&locations.config)
    {
        for program in config.games.iter().flat_map(|g| g.programs.iter()) {
            let current = registry::gpu_preferences(program);
            let wanted = huedra_core::autohdr::edit(
                current.as_deref(),
                huedra_core::autohdr::AutoHdr::Windows,
            );
            if wanted != current {
                let _ = registry::set_gpu_preferences(program, wanted.as_deref());
            }
        }
    }
    println!(
        "Huedra's registry entries were removed (the start-up entry and the \"Start with HDR\" menu entry)."
    );
    if let Ok(locations) = Locations::resolve() {
        let mut folders: Vec<String> = [&locations.config, &locations.log]
            .iter()
            .filter_map(|p| p.parent().map(|d| d.display().to_string()))
            .collect();
        folders.dedup();
        println!(
            "To remove the settings, the log and the caches as well, delete {}.",
            folders.join(" and ")
        );
    }
    Ok(())
}
