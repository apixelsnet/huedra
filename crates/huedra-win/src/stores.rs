//! Finds installed games in all seven stores. Locating the files and keys is
//! done here; parsing is in `huedra_core::library`.
//! Feature doc: `docs/features/04-game-library.md`.

use std::fs;
use std::path::{Path, PathBuf};

use huedra_core::library::{Game, epic, registry as reg, steam, xbox};

use crate::registry::{self, Hive};

/// Store files larger than this are skipped.
const MAX_FILE_BYTES: u64 = 1 << 20;

/// Every installed game found, in store order. Missing stores are skipped.
pub fn scan() -> Vec<Game> {
    let mut games = Vec::new();
    games.extend(steam_games());
    games.extend(epic_games());
    games.extend(registry_games());
    games.extend(xbox_games());
    games.retain(|g| Path::new(&g.install_dir).is_dir());
    games
}

fn read(path: &Path) -> Option<String> {
    huedra_core::files::read_limited(path, MAX_FILE_BYTES)
        .ok()
        .flatten()
}

/// Steam's own folder, when Steam is installed.
pub fn steam_root() -> Option<PathBuf> {
    registry::string(Hive::CurrentUser, r"Software\Valve\Steam", "SteamPath")
        .or_else(|| registry::string(Hive::LocalMachine32, r"SOFTWARE\Valve\Steam", "InstallPath"))
        .map(|root| PathBuf::from(root.replace('/', r"\")))
}

fn steam_games() -> Vec<Game> {
    let Some(root) = steam_root() else {
        return Vec::new();
    };
    let mut libraries = read(&root.join(r"steamapps\libraryfolders.vdf"))
        .map(|text| steam::library_folders(&text))
        .unwrap_or_default();
    if libraries.is_empty() {
        libraries.push(root.to_string_lossy().into_owned());
    }
    let mut games = Vec::new();
    for library in libraries {
        let Ok(entries) = fs::read_dir(Path::new(&library).join("steamapps")) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("appmanifest_")
                && name.ends_with(".acf")
                && let Some(game) =
                    read(&entry.path()).and_then(|t| steam::manifest_game(&t, &library))
            {
                games.push(game);
            }
        }
    }
    games
}

fn epic_games() -> Vec<Game> {
    let Some(data) = std::env::var_os("ProgramData") else {
        return Vec::new();
    };
    let folder = PathBuf::from(data).join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("item"))
        })
        .filter_map(|e| read(&e.path()).and_then(|t| epic::item_game(&t)))
        .collect()
}

fn registry_games() -> Vec<Game> {
    let hive = Hive::LocalMachine32;
    let mut games = Vec::new();
    for entry in registry::subkeys(
        hive,
        r"SOFTWARE\GOG.com\Games",
        &["gameName", "path", "exe", "gameID"],
    ) {
        games.extend(reg::gog_game(&entry));
    }
    for entry in registry::subkeys(hive, r"SOFTWARE\Ubisoft\Launcher\Installs", &["InstallDir"]) {
        games.extend(reg::ubisoft_game(&entry));
    }
    for entry in registry::subkeys(hive, r"SOFTWARE\EA Games", &["Install Dir", "DisplayName"]) {
        games.extend(reg::ea_game(&entry));
    }
    for entry in registry::subkeys(hive, r"SOFTWARE\Rockstar Games", &["InstallFolder"]) {
        games.extend(reg::rockstar_game(&entry));
    }
    games
}

fn xbox_games() -> Vec<Game> {
    let mut games = Vec::new();
    for drive in b'C'..=b'Z' {
        let root = format!(r"{}:\", char::from(drive));
        let Ok(bytes) = fs::read(Path::new(&root).join(".GamingRoot")) else {
            continue;
        };
        for folder in xbox::gaming_root_folders(&bytes) {
            let Ok(entries) = fs::read_dir(Path::new(&root).join(&folder)) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(config) = read(&path.join(r"Content\MicrosoftGame.config")) else {
                    continue;
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                games.extend(xbox::game(&name, &path.to_string_lossy(), &config));
            }
        }
    }
    games
}
