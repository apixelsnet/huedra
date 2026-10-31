//! Command-line grammar. Parsing lives here so it is tested on any platform;
//! the executable only runs what it returns.

use crate::display::{DisplaySelector, HdrState};
use crate::engine::Mode;
use crate::ipc::Request;

/// Help text printed by `huedra --help` and after a usage error.
pub const USAGE: &str = "\
Usage:
  huedra                       Start Huedra in the notification area.
  huedra settings [page]       Open the settings window, optionally on a page:
                               status, games, displays or general.
  huedra status                Show what the running Huedra is doing.
  huedra games                 List the games found in every store.
  huedra mode automatic|hdr|sdr
                               Change the mode of the running Huedra.
  huedra pause [minutes]       Pause automatic switching (default 60).
  huedra resume                Resume automatic switching.
  huedra toggle                Turn HDR on or off, as Win+Alt+H does.
  huedra exit                  Close the running Huedra.
  huedra hdr status            List the displays and their HDR state.
  huedra hdr on  [--display N | all]
  huedra hdr off [--display N | all]
                               Turn HDR on or off. Without --display, the
                               main display is used. N is the number shown
                               under Settings > Display > Identify.
  huedra --version             Show the version.";

/// What the executable was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// No arguments: run in the notification area.
    Tray,
    /// Open the settings window, optionally on a named page.
    Settings(Option<String>),
    /// Print the version.
    Version,
    /// Print the help text.
    Help,
    /// Read or change HDR directly.
    Hdr(HdrCommand),
    /// Ask the running tray process to act.
    Remote(Request),
    /// List the games found in every store.
    Games,
}

/// The `hdr` subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HdrCommand {
    /// List displays and their state.
    Status,
    /// Turn HDR on or off.
    Set {
        /// The state to apply.
        state: HdrState,
        /// The displays to change.
        displays: DisplaySelector,
    },
}

/// The arguments do not form a command.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    /// The first argument is not a command.
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    /// `hdr` without an action.
    #[error("`hdr` needs one of: status, on, off")]
    MissingHdrAction,
    /// An argument the command does not take.
    #[error("unexpected argument `{0}`")]
    UnexpectedArgument(String),
    /// `--display` without a value.
    #[error("`--display` needs a display number or `all`")]
    MissingDisplay,
    /// `--display` with a value that is neither a number nor `all`.
    #[error("`{0}` is not a display number; use 1, 2, ... or `all`")]
    BadDisplay(String),
    /// `mode` without a valid mode.
    #[error("`mode` needs one of: automatic, hdr, sdr")]
    BadMode,
    /// `pause` with a value that is not 1 to 1440 minutes.
    #[error("`{0}` is not a number of minutes from 1 to 1440")]
    BadMinutes(String),
}

/// Parses the arguments after the program name.
///
/// # Errors
/// [`UsageError`] describing the first argument that does not fit.
pub fn parse<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Tray);
    };
    let command = match first.as_str() {
        "--version" | "-V" => Command::Version,
        "--help" | "-h" | "help" | "/?" => Command::Help,
        "settings" => Command::Settings(args.next()),
        "hdr" => Command::Hdr(parse_hdr(&mut args)?),
        "status" => Command::Remote(Request::Status),
        "games" => Command::Games,
        "mode" => Command::Remote(Request::SetMode {
            mode: parse_mode(args.next().as_deref())?,
        }),
        "pause" => Command::Remote(Request::Pause {
            minutes: parse_minutes(args.next())?,
        }),
        "resume" => Command::Remote(Request::Resume),
        "toggle" => Command::Remote(Request::ToggleHdr),
        "exit" => Command::Remote(Request::Exit),
        _ => return Err(UsageError::UnknownCommand(first)),
    };
    match args.next() {
        Some(extra) => Err(UsageError::UnexpectedArgument(extra)),
        None => Ok(command),
    }
}

fn parse_hdr(args: &mut impl Iterator<Item = String>) -> Result<HdrCommand, UsageError> {
    let state = match args.next().as_deref() {
        Some("status") => return Ok(HdrCommand::Status),
        Some("on") => HdrState::On,
        Some("off") => HdrState::Off,
        Some(other) => return Err(UsageError::UnexpectedArgument(other.to_owned())),
        None => return Err(UsageError::MissingHdrAction),
    };
    let displays = match args.next() {
        None => DisplaySelector::Primary,
        Some(flag) if flag == "--display" || flag == "-d" => {
            parse_selector(&args.next().ok_or(UsageError::MissingDisplay)?)?
        }
        Some(other) => return Err(UsageError::UnexpectedArgument(other)),
    };
    Ok(HdrCommand::Set { state, displays })
}

fn parse_mode(value: Option<&str>) -> Result<Mode, UsageError> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("automatic" | "auto") => Ok(Mode::Automatic),
        Some("hdr" | "always-hdr") => Ok(Mode::AlwaysHdr),
        Some("sdr" | "always-sdr") => Ok(Mode::AlwaysSdr),
        _ => Err(UsageError::BadMode),
    }
}

fn parse_minutes(value: Option<String>) -> Result<u32, UsageError> {
    let Some(value) = value else {
        return Ok(60);
    };
    match value.parse::<u32>() {
        Ok(n) if (1..=1440).contains(&n) => Ok(n),
        _ => Err(UsageError::BadMinutes(value)),
    }
}

fn parse_selector(value: &str) -> Result<DisplaySelector, UsageError> {
    if value.eq_ignore_ascii_case("all") {
        return Ok(DisplaySelector::AllCapable);
    }
    match value.parse::<u32>() {
        Ok(n) if n > 0 => Ok(DisplaySelector::Number(n)),
        _ => Err(UsageError::BadDisplay(value.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> Result<Command, UsageError> {
        parse(line.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn plain_commands() {
        assert_eq!(run(""), Ok(Command::Tray), "no arguments");
        assert_eq!(run("--version"), Ok(Command::Version), "version");
        assert_eq!(run("-h"), Ok(Command::Help), "help");
        assert_eq!(run("settings"), Ok(Command::Settings(None)), "settings");
        assert_eq!(
            run("settings games"),
            Ok(Command::Settings(Some("games".into()))),
            "settings page"
        );
    }

    #[test]
    fn hdr_commands() {
        assert_eq!(
            run("hdr status"),
            Ok(Command::Hdr(HdrCommand::Status)),
            "status"
        );
        assert_eq!(
            run("hdr on"),
            Ok(Command::Hdr(HdrCommand::Set {
                state: HdrState::On,
                displays: DisplaySelector::Primary
            })),
            "default display"
        );
        assert_eq!(
            run("hdr off --display 2"),
            Ok(Command::Hdr(HdrCommand::Set {
                state: HdrState::Off,
                displays: DisplaySelector::Number(2)
            })),
            "numbered display"
        );
        assert_eq!(
            run("hdr on -d ALL"),
            Ok(Command::Hdr(HdrCommand::Set {
                state: HdrState::On,
                displays: DisplaySelector::AllCapable
            })),
            "all displays"
        );
    }

    #[test]
    fn remote_commands() {
        assert_eq!(
            run("status"),
            Ok(Command::Remote(Request::Status)),
            "status"
        );
        assert_eq!(
            run("mode hdr"),
            Ok(Command::Remote(Request::SetMode {
                mode: Mode::AlwaysHdr
            })),
            "mode"
        );
        assert_eq!(
            run("pause"),
            Ok(Command::Remote(Request::Pause { minutes: 60 })),
            "default pause"
        );
        assert_eq!(
            run("pause 15"),
            Ok(Command::Remote(Request::Pause { minutes: 15 })),
            "pause"
        );
        assert_eq!(run("mode"), Err(UsageError::BadMode), "missing mode");
        assert_eq!(
            run("pause 0"),
            Err(UsageError::BadMinutes("0".into())),
            "zero minutes"
        );
        assert_eq!(run("exit"), Ok(Command::Remote(Request::Exit)), "exit");
    }

    #[test]
    fn usage_errors() {
        assert_eq!(
            run("frobnicate"),
            Err(UsageError::UnknownCommand("frobnicate".into())),
            "unknown"
        );
        assert_eq!(run("hdr"), Err(UsageError::MissingHdrAction), "no action");
        assert_eq!(
            run("hdr on --display"),
            Err(UsageError::MissingDisplay),
            "no value"
        );
        assert_eq!(
            run("hdr on --display 0"),
            Err(UsageError::BadDisplay("0".into())),
            "zero"
        );
        assert_eq!(
            run("hdr status extra"),
            Err(UsageError::UnexpectedArgument("extra".into())),
            "trailing"
        );
    }
}
