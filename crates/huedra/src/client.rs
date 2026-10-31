//! Commands that ask the running tray process to act, such as
//! `huedra mode` and `huedra status`. Feature doc: `docs/features/09-cli-ipc.md`.

use huedra_core::ipc::{Request, Response, StatusReport};
use huedra_win::pipe;

/// Sends one request and prints the answer.
pub fn run(request: &Request) -> anyhow::Result<()> {
    match pipe::call(request)? {
        Response::Done { message } => println!("{message}"),
        Response::Status(report) => print!("{}", describe(&report)),
        Response::Failed { message } => anyhow::bail!(message),
    }
    Ok(())
}

fn describe(report: &StatusReport) -> String {
    let mut out = format!("{}\nMode: {}\n", report.summary, report.mode.label());
    if let Some(minutes) = report.paused_minutes {
        out.push_str(&format!("Paused: {minutes} more minutes\n"));
    }
    for display in &report.displays {
        out.push_str(&format!("{}: {}\n", display.label, display.detail));
    }
    if !report.history.is_empty() {
        out.push_str("\nRecent changes:\n");
        for entry in report.history.iter().take(10) {
            out.push_str(&format!(
                "  {}  {}: {}\n",
                entry.time, entry.title, entry.detail
            ));
        }
    }
    out
}
