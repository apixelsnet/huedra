//! The Xbox Game Bar's list of games: Windows records each program it
//! recognized as a game while it ran, under
//! `HKCU\System\GameConfigStore\Children\<guid>` (`MatchedExeFullPath`).
//! This finds games that no launcher lists (a standalone install, a game
//! from a launcher Huedra does not read). `huedra-win` reads the key and
//! leaves out programs inside a game that a launcher already lists.

use super::{Game, GameId, Store, is_helper_program};
use crate::paths::NormPath;

/// The key whose subkeys hold the recognized programs.
pub const CHILDREN_KEY: &str = r"System\GameConfigStore\Children";
/// The value naming each program.
pub const PATH_VALUE: &str = "MatchedExeFullPath";

/// Programs that the Game Bar can list but that are launchers, stores or
/// runtimes rather than games (file names without `.exe`, lower case).
const NOT_GAMES: &[&str] = &[
    "java",
    "javaw",
    "launcher",
    "steam",
    "steamwebhelper",
    "epicgameslauncher",
    "galaxyclient",
    "upc",
    "ubisoftconnect",
    "eadesktop",
    "origin",
    "battle.net",
    "riotclientservices",
    "riotclientux",
    "leagueclient",
    "leagueclientux",
    "amazon games ui",
    "itch",
    "heroic",
    "hoyoplay",
    "xboxpcapp",
    "playnite.desktopapp",
    "playnite.fullscreenapp",
];

/// Folders inside a game folder that do not name the game
/// (`Binaries\Win64`, `Versions\version-…`, `_retail_`).
fn is_generic(folder: &str) -> bool {
    const GENERIC: &[&str] = &[
        "bin",
        "bin32",
        "bin64",
        "binaries",
        "client",
        "current",
        "game",
        "live",
        "release",
        "retail",
        "retailclient",
        "shipping",
        "system",
        "versions",
        "win32",
        "win64",
        "x64",
        "x86",
    ];
    GENERIC.contains(&folder)
        || folder.starts_with('_')
        || folder.starts_with("version-")
        || folder.starts_with("app-")
}

/// Folders that hold many programs, so they never name a game.
fn is_shared(folder: &str) -> bool {
    const SHARED: &[&str] = &[
        "appdata",
        "common",
        "epic games",
        "games",
        "gog games",
        "local",
        "packages",
        "program files",
        "program files (x86)",
        "programdata",
        "roaming",
        "steamapps",
        "users",
        "windowsapps",
        "xboxgames",
    ];
    folder.ends_with(':') || SHARED.contains(&folder)
}

/// The game for one recognized program: named after its game folder, the
/// first folder above it that is not a generic one such as `Binaries` or
/// `Win64` (`Roblox\Versions\version-1\RobloxPlayerBeta.exe` is Roblox).
/// That folder is the install folder. Launchers, runtimes, helpers,
/// programs in the Windows folder or a `Temp` folder, and programs with no
/// game folder of their own (directly in `Games`, for example) are left out.
pub fn game(exe_path: &str) -> Option<Game> {
    let exe = exe_path.trim().trim_matches('"').replace('/', r"\");
    let parts: Vec<&str> = exe.split('\\').filter(|p| !p.is_empty()).collect();
    let (file, folders) = parts.split_last()?;
    let lower = file.to_lowercase();
    let stem = lower.strip_suffix(".exe")?;
    let in_windows = folders
        .get(1)
        .is_some_and(|f| f.eq_ignore_ascii_case("windows"));
    let in_temp = folders.iter().any(|f| f.eq_ignore_ascii_case("temp"));
    if stem.is_empty()
        || in_windows
        || in_temp
        || is_helper_program(&lower)
        || NOT_GAMES.contains(&stem)
    {
        return None;
    }
    let at = folders
        .iter()
        .rposition(|f| !is_generic(&f.to_lowercase()))?;
    let name = folders.get(at)?;
    if at == 0 || is_shared(&name.to_lowercase()) {
        return None;
    }
    Some(Game {
        id: GameId::new(Store::GameBar, NormPath::new(&exe).as_str()),
        name: (*name).to_owned(),
        store: Store::GameBar,
        install_dir: folders.get(..=at)?.join(r"\"),
        exe: Some(exe),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(exe: &str) -> Option<(String, String)> {
        game(exe).map(|g| (g.name, g.install_dir))
    }

    #[test]
    fn names_after_the_game_folder() {
        assert_eq!(
            named(r"C:\Users\x\AppData\Local\Roblox\Versions\version-80c7\RobloxPlayerBeta.exe"),
            Some(("Roblox".into(), r"C:\Users\x\AppData\Local\Roblox".into())),
            "generic folders skipped"
        );
        assert_eq!(
            named(r"D:\Games\Some Game\Some\Binaries\Win64\Some-Win64-Shipping.exe"),
            Some(("Some".into(), r"D:\Games\Some Game\Some".into())),
            "unreal layout"
        );
        assert_eq!(
            named(r"C:\Program Files\Epic Games\ShadowoftheTombRaider\SOTTR.exe"),
            Some((
                "ShadowoftheTombRaider".into(),
                r"C:\Program Files\Epic Games\ShadowoftheTombRaider".into()
            )),
            "plain folder"
        );
        let found = game(r#""E:\Indie\Game.exe""#).expect("game");
        assert_eq!(
            (found.id.as_str(), found.exe.as_deref()),
            (r"gamebar:e:\indie\game.exe", Some(r"E:\Indie\Game.exe")),
            "id from the program path"
        );
    }

    #[test]
    fn leaves_out_what_is_not_a_game() {
        for exe in [
            r"C:\Users\x\AppData\Local\Packages\Microsoft.X\LocalCache\runtime\bin\javaw.exe",
            r"C:\Windows\System32\notepad.exe",
            r"C:\Users\x\AppData\Local\Temp\test\apps\Tool.exe",
            r"D:\Games\Game.exe",
            r"C:\Game.exe",
            r"D:\Games\X\UnityCrashHandler64.exe",
            r"E:\Riot Games\League of Legends\LeagueClientUx.exe",
            r"D:\Games\X\x.bin",
            "",
        ] {
            assert!(game(exe).is_none(), "{exe}");
        }
    }
}
