//! The `huedra` executable: tray process, settings window and command line.
//!
//! Phase 0 only reports its version; see `docs/features/09-cli-ipc.md`.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--version" | "-V") => {
            println!("{} {}", huedra_core::APP_NAME, huedra_core::VERSION);
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown argument: {other}");
            ExitCode::from(2)
        }
    }
}
