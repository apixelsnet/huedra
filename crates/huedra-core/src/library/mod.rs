//! Installed games and how a running program is matched to one.
//! Feature doc: `docs/features/04-game-library.md`.

pub mod epic;
pub mod registry;
pub mod steam;
pub mod vdf;
pub mod xbox;

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::paths::NormPath;

/// Where a game came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Store {
    /// Steam.
    Steam,
    /// Epic Games Store.
    Epic,
    /// GOG Galaxy.
    Gog,
    /// Xbox app (Microsoft Store).
    Xbox,
    /// Ubisoft Connect.
    Ubisoft,
    /// EA app.
    Ea,
    /// Rockstar Games Launcher.
    Rockstar,
    /// Added by the user.
    Manual,
}

impl Store {
    /// Name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Steam => "Steam",
            Self::Epic => "Epic",
            Self::Gog => "GOG",
            Self::Xbox => "Xbox",
            Self::Ubisoft => "Ubisoft",
            Self::Ea => "EA",
            Self::Rockstar => "Rockstar",
            Self::Manual => "Added manually",
        }
    }

    /// Prefix used in [`GameId`]s.
    pub fn key(self) -> &'static str {
        match self {
            Self::Steam => "steam",
            Self::Epic => "epic",
            Self::Gog => "gog",
            Self::Xbox => "xbox",
            Self::Ubisoft => "ubisoft",
            Self::Ea => "ea",
            Self::Rockstar => "rockstar",
            Self::Manual => "manual",
        }
    }
}

/// Stable identity of a game: store key and the store's own id, for example
/// `steam:1091500`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(String);

impl GameId {
    /// Builds an id from a store and the store's id.
    pub fn new(store: Store, id: &str) -> Self {
        Self(format!("{}:{}", store.key(), id.trim().to_lowercase()))
    }

    /// Wraps an id read from a file.
    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The text form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The Steam app id, when this is a Steam game.
    pub fn steam_app_id(&self) -> Option<u32> {
        self.0.strip_prefix("steam:")?.parse().ok()
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An installed game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// Stable identity.
    pub id: GameId,
    /// Title as the store shows it.
    pub name: String,
    /// Source.
    pub store: Store,
    /// Install folder. Any program inside it counts as the game.
    pub install_dir: String,
    /// The main program, when known (always set for manual entries).
    pub exe: Option<String>,
}

/// File names inside game folders that are never the game itself.
const NOT_GAMES: &[&str] = &[
    "unins",
    "crashreport",
    "crashhandler",
    "crashpad",
    "vc_redist",
    "vcredist",
    "dxsetup",
    "dotnetfx",
    "ue4prereqsetup",
    "easyanticheat_setup",
    "unitycrashhandler",
];

/// Whether a program's file name (lower case) is an installer, uninstaller
/// or crash reporter rather than a game.
pub fn is_helper_program(file_name: &str) -> bool {
    NOT_GAMES.iter().any(|n| file_name.starts_with(n))
}

/// A set of games prepared for fast matching.
#[derive(Debug, Default, Clone)]
pub struct Library {
    games: Vec<Game>,
    folders: Vec<NormPath>,
    exes: Vec<Option<NormPath>>,
}

impl Library {
    /// Builds the matcher. Later entries with an id already seen are ignored.
    pub fn new(games: Vec<Game>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let games: Vec<Game> = games
            .into_iter()
            .filter(|g| seen.insert(g.id.clone()))
            .collect();
        let folders = games
            .iter()
            .map(|g| NormPath::new(&g.install_dir))
            .collect();
        let exes = games
            .iter()
            .map(|g| g.exe.as_deref().map(NormPath::new))
            .collect();
        Self {
            games,
            folders,
            exes,
        }
    }

    /// All games.
    pub fn games(&self) -> &[Game] {
        &self.games
    }

    /// Looks up a game by id.
    pub fn get(&self, id: &GameId) -> Option<&Game> {
        self.games.iter().find(|g| &g.id == id)
    }

    /// The game a running program belongs to: an exact program match first,
    /// otherwise the game with the deepest install folder containing it.
    pub fn find(&self, exe_path: &str) -> Option<&Game> {
        let exe = NormPath::new(exe_path);
        let name = exe.file_name();
        if is_helper_program(name) {
            return None;
        }
        if let Some(i) = self.exes.iter().position(|e| e.as_ref() == Some(&exe)) {
            return self.games.get(i);
        }
        self.folders
            .iter()
            .enumerate()
            .filter(|(_, folder)| folder.as_str().len() > 3 && exe.is_within(folder))
            .max_by_key(|(_, folder)| folder.as_str().len())
            .and_then(|(i, _)| self.games.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(store: Store, id: &str, dir: &str, exe: Option<&str>) -> Game {
        Game {
            id: GameId::new(store, id),
            name: id.to_owned(),
            store,
            install_dir: dir.to_owned(),
            exe: exe.map(str::to_owned),
        }
    }

    #[test]
    fn finds_by_folder_and_exe() {
        let library = Library::new(vec![
            game(
                Store::Steam,
                "1091500",
                r"D:\Steam\common\Cyberpunk 2077",
                None,
            ),
            game(Store::Steam, "1", r"D:\Steam\common", None),
            game(
                Store::Manual,
                "tool",
                r"C:\Tools",
                Some(r"C:\Tools\Game.exe"),
            ),
        ]);
        let found = |p: &str| library.find(p).map(|g| g.id.as_str().to_owned());
        assert_eq!(
            found(r"d:\steam\common\cyberpunk 2077\bin\x64\Cyberpunk2077.exe"),
            Some("steam:1091500".into()),
            "deepest folder wins"
        );
        assert_eq!(
            found(r"C:\Tools\Game.exe"),
            Some("manual:tool".into()),
            "exact program"
        );
        assert_eq!(
            found(r"C:\Tools\Other.exe"),
            Some("manual:tool".into()),
            "folder of manual entry"
        );
        assert_eq!(found(r"C:\Windows\explorer.exe"), None, "unrelated");
        assert_eq!(
            found(r"D:\Steam\common\Cyberpunk 2077\unins000.exe"),
            None,
            "uninstaller ignored"
        );
    }

    #[test]
    fn ids() {
        let id = GameId::new(Store::Steam, "1091500");
        assert_eq!(id.as_str(), "steam:1091500", "format");
        assert_eq!(id.steam_app_id(), Some(1_091_500), "steam id");
        assert_eq!(
            GameId::new(Store::Epic, "Fn").steam_app_id(),
            None,
            "not steam"
        );
    }
}
