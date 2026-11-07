//! Command-line grammar. Parsing lives here so it is tested on any platform;
//! the executable only runs what it returns.

use crate::display::{DisplaySelector, HdrState};
use crate::engine::Mode;
use crate::ipc::Request;
use crate::launch::{self, LaunchCommand};

/// Help text printed by `huedra --help` and after a usage error.
pub const USAGE: &str = "\
Usage:
  huedra                       Start Huedra and open its window. When Huedra
                               is running already, its window comes to the
                               front.
  huedra --background          Start Huedra in the tray without opening the
                               window, as Start with Windows does.
  huedra settings [page]       Open the settings window, optionally on a page:
                               status, games, video, displays or settings.
  huedra flyout                Open the quick panel, as a click on the tray
                               icon does.
  huedra status                Show what the running Huedra is doing.
  huedra games                 List the games found in every store.
  huedra mode automatic|hdr|sdr
                               Change the mode of the running Huedra.
  huedra pause [minutes]       Pause automatic switching (default 60).
  huedra resume                Resume automatic switching.
  huedra toggle                Turn HDR on or off, as Win+Alt+H does.
  huedra exit                  Close the running Huedra.
  huedra launch -- <program> [arguments]
                               Turn HDR on, start the program, and keep HDR
                               on until it ends. As a Steam launch option:
                               \"<path to huedra.exe>\" launch -- %command%
  huedra uninstall             Close Huedra and remove everything it added
                               to the registry.
  huedra update [--check] [--from <folder>]
                               Install a newer signed release now, or with
                               --check only report it. --from reads the
                               release files from a folder instead of the
                               release address.
  huedra hdr status            List the displays and their HDR state.
  huedra hdr on  [--display N | all]
  huedra hdr off [--display N | all]
                               Turn HDR on or off. Without --display, the
                               main display is used. N is the number shown
                               under Settings > System > Display >
                               Identify.
  huedra --version             Show the version.";

/// What the executable was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// No arguments, or `--background`: run in the notification area.
    Tray(TrayStart),
    /// Open the settings window, optionally on a named page.
    Settings(Option<String>),
    /// Open the quick panel, beside the tray icon at the given place (left,
    /// top, right, bottom in physical pixels) or beside the pointer.
    Flyout(Option<crate::flyout::Rect>),
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
    /// Start a program with HDR on.
    Launch(LaunchCommand),
    /// Remove every registry entry Huedra wrote.
    Uninstall,
    /// Check for, install or roll back an update.
    Update(UpdateCommand),
}

/// Whether the tray opens the settings window when it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStart {
    /// Started by the user: open the window too.
    Window,
    /// Started by Windows, an update or `launch`: the notification area only.
    Background,
}

/// The argument that starts the tray without opening the window.
pub const BACKGROUND: &str = "--background";

/// The command the start-up entry holds for this program: the quoted path
/// and [`BACKGROUND`], so signing in to Windows does not open the window.
#[must_use]
pub fn startup_command(exe: &std::path::Path) -> String {
    format!("\"{}\" {BACKGROUND}", exe.display())
}

/// The program a start-up entry names: the quoted path, or everything up
/// to the first space when the path is not quoted.
#[must_use]
pub fn startup_program(command: &str) -> &str {
    let command = command.trim();
    match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        None => command.split(' ').next().unwrap_or(command),
    }
}

/// Whether a start-up entry starts `exe` (ignoring case and the form of
/// the path), rather than another copy of Huedra.
#[must_use]
pub fn startup_starts(command: &str, exe: &std::path::Path) -> bool {
    crate::paths::NormPath::new(startup_program(command))
        == crate::paths::NormPath::new(&exe.display().to_string())
}

/// The `update` forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCommand {
    /// Check, and install a newer release unless `check_only`.
    Run {
        /// Only report what is available.
        check_only: bool,
        /// A folder holding the release files, instead of the release
        /// address.
        from: Option<String>,
    },
    /// The daily check started by the tray, which follows the Updates
    /// setting.
    Auto,
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
    /// `launch` without a program.
    #[error("`launch` needs a program: huedra launch -- <program> [arguments]")]
    MissingProgram,
    /// `launch` with a batch file, which would run through `cmd.exe`.
    #[error("`{0}` is a batch file; `launch` starts programs only")]
    BatchFile(String),
    /// `--from` without a folder.
    #[error("`--from` needs a folder")]
    MissingFolder,
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
        return Ok(Command::Tray(TrayStart::Window));
    };
    let command = match first.as_str() {
        BACKGROUND => Command::Tray(TrayStart::Background),
        "--version" | "-V" => Command::Version,
        "--help" | "-h" | "help" | "/?" => Command::Help,
        "settings" => Command::Settings(args.next()),
        "flyout" => {
            let rest: Vec<String> = args.collect();
            if rest.is_empty() {
                return Ok(Command::Flyout(None));
            }
            return crate::flyout::Rect::parse(&rest)
                .map(|rect| Command::Flyout(Some(rect)))
                .ok_or_else(|| UsageError::UnexpectedArgument(rest.join(" ")));
        }
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
        "uninstall" => Command::Uninstall,
        "update" => return parse_update(args).map(Command::Update),
        // Everything after `launch` belongs to the program.
        "launch" => return parse_launch(args).map(Command::Launch),
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

fn parse_launch(args: impl Iterator<Item = String>) -> Result<LaunchCommand, UsageError> {
    let mut args = args.peekable();
    if args.peek().is_some_and(|a| a == "--") {
        args.next();
    }
    let program = args.next().ok_or(UsageError::MissingProgram)?;
    if program.trim().is_empty() {
        return Err(UsageError::MissingProgram);
    }
    if launch::is_batch_file(&program) {
        return Err(UsageError::BatchFile(program));
    }
    Ok(LaunchCommand {
        program,
        args: args.collect(),
    })
}

fn parse_update(mut args: impl Iterator<Item = String>) -> Result<UpdateCommand, UsageError> {
    let first = args.next();
    let command = match first.as_deref() {
        Some("--auto") => UpdateCommand::Auto,
        _ => return parse_update_run(first.into_iter().chain(args)),
    };
    match args.next() {
        Some(extra) => Err(UsageError::UnexpectedArgument(extra)),
        None => Ok(command),
    }
}

fn parse_update_run(mut args: impl Iterator<Item = String>) -> Result<UpdateCommand, UsageError> {
    let (mut check_only, mut from) = (false, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" if !check_only => check_only = true,
            "--from" if from.is_none() => {
                let folder = args.next().filter(|f| !f.trim().is_empty());
                from = Some(folder.ok_or(UsageError::MissingFolder)?);
            }
            _ => return Err(UsageError::UnexpectedArgument(arg)),
        }
    }
    Ok(UpdateCommand::Run { check_only, from })
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
        assert_eq!(
            run(""),
            Ok(Command::Tray(TrayStart::Window)),
            "no arguments"
        );
        assert_eq!(
            run("--background"),
            Ok(Command::Tray(TrayStart::Background)),
            "background"
        );
        assert_eq!(
            run("--background now"),
            Err(UsageError::UnexpectedArgument("now".into())),
            "background takes nothing"
        );
        assert_eq!(run("--version"), Ok(Command::Version), "version");
        assert_eq!(run("-h"), Ok(Command::Help), "help");
        assert_eq!(run("settings"), Ok(Command::Settings(None)), "settings");
        assert_eq!(run("flyout"), Ok(Command::Flyout(None)), "flyout");
        assert_eq!(
            run("flyout 1 2 3 4"),
            Ok(Command::Flyout(Some(crate::flyout::Rect {
                left: 1,
                top: 2,
                right: 3,
                bottom: 4
            }))),
            "flyout at the icon"
        );
        assert!(run("flyout 1 2").is_err(), "flyout needs four numbers");
        assert_eq!(
            run("settings games"),
            Ok(Command::Settings(Some("games".into()))),
            "settings page"
        );
    }

    #[test]
    fn startup_entry_command() {
        let exe = std::path::Path::new(r"C:\Program Files\Huedra\huedra.exe");
        let command = startup_command(exe);
        assert_eq!(
            command, r#""C:\Program Files\Huedra\huedra.exe" --background"#,
            "quoted path and the background argument"
        );
        assert_eq!(
            startup_program(&command),
            r"C:\Program Files\Huedra\huedra.exe",
            "program of a new entry"
        );
        assert_eq!(
            startup_program(r#" "D:\Huedra\huedra.exe" "#),
            r"D:\Huedra\huedra.exe",
            "program of an entry from before the argument"
        );
        assert_eq!(
            startup_program(r"D:\Huedra\huedra.exe --background"),
            r"D:\Huedra\huedra.exe",
            "unquoted path"
        );
        assert!(
            startup_starts(
                &command,
                std::path::Path::new(r"c:\program files\huedra\HUEDRA.EXE")
            ),
            "same program, other case"
        );
        assert!(
            !startup_starts(&command, std::path::Path::new(r"D:\Portable\huedra.exe")),
            "another copy"
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
    fn launch_commands() {
        let launch = |args: &[&str]| parse(args.iter().map(|a| (*a).to_owned()));
        assert_eq!(
            launch(&[
                "launch",
                "--",
                r"D:\Games\Foo\foo.exe",
                "-dx12",
                "--",
                "x y"
            ]),
            Ok(Command::Launch(LaunchCommand {
                program: r"D:\Games\Foo\foo.exe".into(),
                args: vec!["-dx12".into(), "--".into(), "x y".into()],
            })),
            "arguments pass through unchanged"
        );
        assert_eq!(
            launch(&["launch", "foo.exe"]),
            Ok(Command::Launch(LaunchCommand {
                program: "foo.exe".into(),
                args: Vec::new(),
            })),
            "without --"
        );
        assert_eq!(
            launch(&["launch", "--"]),
            Err(UsageError::MissingProgram),
            "no program"
        );
        assert_eq!(
            launch(&["launch", "--", "start.bat"]),
            Err(UsageError::BatchFile("start.bat".into())),
            "batch file"
        );
        assert_eq!(run("uninstall"), Ok(Command::Uninstall), "uninstall");
    }

    #[test]
    fn update_commands() {
        let update = |args: &[&str]| parse(args.iter().map(|a| (*a).to_owned()));
        assert_eq!(
            update(&["update"]),
            Ok(Command::Update(UpdateCommand::Run {
                check_only: false,
                from: None
            })),
            "install now"
        );
        assert_eq!(
            update(&["update", "--from", r"D:\Huedra 0.2.0", "--check"]),
            Ok(Command::Update(UpdateCommand::Run {
                check_only: true,
                from: Some(r"D:\Huedra 0.2.0".into())
            })),
            "check a folder"
        );
        assert_eq!(
            update(&["update", "--rollback"]),
            Err(UsageError::UnexpectedArgument("--rollback".into())),
            "no rollback"
        );
        assert_eq!(
            update(&["update", "--auto"]),
            Ok(Command::Update(UpdateCommand::Auto)),
            "auto"
        );
        assert_eq!(
            update(&["update", "--from"]),
            Err(UsageError::MissingFolder),
            "no folder"
        );
        assert_eq!(
            update(&["update", "--auto", "--check"]),
            Err(UsageError::UnexpectedArgument("--check".into())),
            "auto takes nothing"
        );
        assert_eq!(
            update(&["update", "--check", "--check"]),
            Err(UsageError::UnexpectedArgument("--check".into())),
            "repeated"
        );
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
