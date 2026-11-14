//! `huedra hdr status | on | off | brightness`.
//! Feature doc: `docs/features/01-hdr-switch.md`.

use anyhow::Context as _;
use huedra_core::command::HdrCommand;
use huedra_core::display::{Display, HdrState, HdrSupport};
use huedra_win::display;

/// One try plus the default two retries.
const ATTEMPTS: u32 = 3;

/// Runs one `hdr` subcommand.
pub fn run(command: HdrCommand) -> anyhow::Result<()> {
    let all = display::displays().context("could not list the displays")?;
    match command {
        HdrCommand::Status => print!("{}", status_table(&all)),
        HdrCommand::Brightness { nits, displays } => {
            for chosen in displays.pick(&all)? {
                if let Some(nits) = nits {
                    display::set_sdr_brightness(&chosen.id, nits)?;
                }
                if chosen.state == HdrState::On {
                    let now = display::sdr_brightness(&chosen.id)?;
                    println!(
                        "Display {}  {}: SDR content brightness {now}",
                        chosen.number,
                        chosen.label()
                    );
                } else {
                    println!(
                        "Display {}  {}: HDR off; SDR content brightness shows while HDR is on",
                        chosen.number,
                        chosen.label()
                    );
                }
            }
        }
        HdrCommand::Set { state, displays } => {
            for chosen in displays.pick(&all)? {
                display::set_hdr(&chosen.id, state, ATTEMPTS)?;
                println!("Display {}  {}: HDR {state}", chosen.number, chosen.label());
            }
        }
    }
    Ok(())
}

fn status_table(displays: &[Display]) -> String {
    if displays.is_empty() {
        return "No active display was found.\n".to_owned();
    }
    let width = displays
        .iter()
        .map(|d| d.label().chars().count())
        .max()
        .unwrap_or(0);
    displays
        .iter()
        .map(|d| {
            let hdr = match (d.support, d.state) {
                (HdrSupport::Unsupported, _) => "HDR not supported",
                (_, HdrState::On) => "HDR on",
                (_, HdrState::Off) => "HDR off",
            };
            let mut notes = Vec::new();
            if d.primary {
                notes.push("main display".to_owned());
            }
            if d.support == HdrSupport::LimitedByPolicy {
                notes.push("limited by Windows policy".to_owned());
            }
            if let Some(peak) = d.peak {
                notes.push(format!("peak {peak}"));
            }
            let line = format!(
                "Display {}  {:width$}  {hdr:17}  {}",
                d.number,
                d.label(),
                notes.join(", ")
            );
            format!("{}\n", line.trim_end())
        })
        .collect()
}
