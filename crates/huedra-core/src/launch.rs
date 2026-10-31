//! Launch with HDR: `huedra launch -- <program> [arguments]` turns HDR on,
//! starts the program, and keeps HDR on until it and the programs it started
//! from its folder have ended. Feature doc: `docs/features/06-launch-with-hdr.md`.

use crate::engine::TriggerId;
use crate::paths::NormPath;

const TRIGGER_PREFIX: &str = "launch:";

/// What `huedra launch` was asked to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    /// The program, as given.
    pub program: String,
    /// Its arguments, passed on unchanged.
    pub args: Vec<String>,
}

/// The trigger of a launch, keyed by the process id of `huedra launch`
/// (the tray holds HDR while that process lives).
pub fn trigger_id(launcher_pid: u32) -> TriggerId {
    TriggerId::new(format!("{TRIGGER_PREFIX}{launcher_pid}"))
}

/// Whether a trigger belongs to a launch rather than a game or a video.
pub fn is_launch(id: &TriggerId) -> bool {
    id.as_str().starts_with(TRIGGER_PREFIX)
}

/// Whether a program would be run through `cmd.exe` (batch files), which
/// `huedra launch` refuses.
pub fn is_batch_file(program: &str) -> bool {
    let path = NormPath::new(program);
    let name = path.file_name();
    name.ends_with(".bat") || name.ends_with(".cmd")
}

/// The folder whose programs keep the launch going after the started
/// program exits: the working folder when the program lies inside it (Steam
/// starts games in their install folder), otherwise the program's own folder.
/// A drive root is never used as the working folder.
pub fn wait_folder(program: &str, working_dir: &str) -> NormPath {
    let program = NormPath::new(program);
    let cwd = NormPath::new(working_dir);
    if cwd.as_str().len() > 3 && program.is_within(&cwd) && program != cwd {
        return cwd;
    }
    program.parent().unwrap_or(program)
}

/// Steam launch option: Steam replaces `%command%` with the game and its
/// arguments.
pub fn steam_option(huedra_exe: &str) -> String {
    format!("\"{huedra_exe}\" launch -- %command%")
}

/// Arguments of a desktop shortcut that starts `program` with HDR.
pub fn shortcut_arguments(program: &str) -> String {
    format!("launch -- \"{program}\"")
}

/// Command of the "Start with HDR" entry in the menu of programs in
/// Explorer; `%1` is the program clicked.
pub fn context_menu_command(huedra_exe: &str) -> String {
    format!("\"{huedra_exe}\" launch -- \"%1\"")
}

/// File name of the desktop shortcut that starts a game with HDR, without
/// the characters Windows does not allow in file names.
pub fn shortcut_file_name(game: &str) -> String {
    let name: String = game
        .chars()
        .filter(|c| {
            !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()
        })
        .collect();
    let name = name.trim().trim_end_matches('.');
    let name = if name.is_empty() { "Game" } else { name };
    format!("{name} (with HDR).lnk")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_ids() {
        let id = trigger_id(4242);
        assert_eq!(id.as_str(), "launch:4242", "key");
        assert!(is_launch(&id), "recognised");
        assert!(!is_launch(&TriggerId::new("steam:1091500")), "a game");
    }

    #[test]
    fn batch_files() {
        assert!(is_batch_file(r"C:\Games\Start.BAT"), "bat");
        assert!(is_batch_file("run.cmd"), "cmd");
        assert!(!is_batch_file(r"C:\Games\Game.exe"), "exe");
    }

    #[test]
    fn wait_folders() {
        assert_eq!(
            wait_folder(
                r"D:\Steam\common\Game\bin\x64\game.exe",
                r"D:\Steam\common\Game"
            )
            .as_str(),
            r"d:\steam\common\game",
            "install folder from Steam"
        );
        assert_eq!(
            wait_folder(r"D:\Games\Foo\foo.exe", r"C:\Users\someone").as_str(),
            r"d:\games\foo",
            "program folder when started elsewhere"
        );
        assert_eq!(
            wait_folder(r"D:\Games\Foo\foo.exe", r"D:\").as_str(),
            r"d:\games\foo",
            "a drive root is never the working folder"
        );
    }

    #[test]
    fn shortcut_names() {
        assert_eq!(
            shortcut_file_name("Cyberpunk 2077"),
            "Cyberpunk 2077 (with HDR).lnk",
            "plain"
        );
        assert_eq!(
            shortcut_file_name("Star Wars: Outlaws?"),
            "Star Wars Outlaws (with HDR).lnk",
            "forbidden characters"
        );
        assert_eq!(
            shortcut_file_name(" ... "),
            "Game (with HDR).lnk",
            "nothing left"
        );
    }

    #[test]
    fn command_texts() {
        assert_eq!(
            steam_option(r"C:\Tools\huedra.exe"),
            r#""C:\Tools\huedra.exe" launch -- %command%"#,
            "steam"
        );
        assert_eq!(
            shortcut_arguments(r"D:\Games\Foo\foo.exe"),
            r#"launch -- "D:\Games\Foo\foo.exe""#,
            "shortcut"
        );
        assert_eq!(
            context_menu_command(r"C:\Tools\huedra.exe"),
            r#""C:\Tools\huedra.exe" launch -- "%1""#,
            "context menu"
        );
    }
}
