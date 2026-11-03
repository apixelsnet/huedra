//! Finds installed games in every store and launcher Huedra reads, then in
//! the Xbox Game Bar's list. Locating the files and keys is done here;
//! parsing is in `huedra_core::library`.
//! Feature doc: `docs/features/04-game-library.md`.

use std::fs;
use std::path::{Path, PathBuf};

use huedra_core::library::{
    Game, amazon, epic, gamebar, heroic, itch, registry as reg, steam, xbox,
};
use huedra_core::paths::NormPath;

use crate::registry::{self, Hive};
use crate::sqlite;

/// Store files larger than this are skipped.
const MAX_FILE_BYTES: u64 = 1 << 20;

/// Installed programs, machine-wide (both views) and for this user (where
/// the Riot Client registers its games).
const UNINSTALL_KEYS: [(Hive, &str); 3] = [
    (
        Hive::LocalMachine32,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        Hive::LocalMachine,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        Hive::CurrentUser,
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
];

/// Every installed game found, in store order, then the Game Bar's games
/// that no store lists. Missing stores are skipped.
pub fn scan() -> Vec<Game> {
    let mut games = Vec::new();
    games.extend(steam_games());
    games.extend(epic_games());
    games.extend(registry_games());
    games.extend(xbox_games());
    games.extend(uninstall_games());
    games.extend(hoyoplay_games());
    games.extend(amazon_games());
    games.extend(itch_games());
    games.extend(heroic_games());
    games.retain(|g| Path::new(&g.install_dir).is_dir());
    let found = game_bar_games(&games);
    games.extend(found);
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

/// The Epic Games Launcher's manifest folder: under the data folder it
/// records in the registry (`AppDataPath`), otherwise the default one in
/// `%ProgramData%`.
fn epic_manifests() -> Option<PathBuf> {
    registry::string(
        Hive::LocalMachine32,
        r"SOFTWARE\Epic Games\EpicGamesLauncher",
        "AppDataPath",
    )
    .map(|data| PathBuf::from(data.trim()).join("Manifests"))
    .filter(|folder| folder.is_dir())
    .or_else(|| {
        known_folder("ProgramData").map(|data| data.join(r"Epic\EpicGamesLauncher\Data\Manifests"))
    })
}

fn epic_games() -> Vec<Game> {
    let Some(folder) = epic_manifests() else {
        return Vec::new();
    };
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

/// Battle.net and Riot Games, from the installed programs' uninstall
/// entries.
fn uninstall_games() -> Vec<Game> {
    let mut games = Vec::new();
    for (hive, path) in UNINSTALL_KEYS {
        for entry in registry::subkeys(hive, path, reg::UNINSTALL_VALUES) {
            games.extend(reg::battlenet_game(&entry).or_else(|| reg::riot_game(&entry)));
        }
    }
    games
}

fn hoyoplay_games() -> Vec<Game> {
    reg::HOYOPLAY_KEYS
        .iter()
        .flat_map(|path| registry::subkeys(Hive::CurrentUser, path, reg::HOYOPLAY_VALUES))
        .filter_map(|entry| reg::hoyoplay_game(&entry))
        .collect()
}

fn known_folder(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable).map(PathBuf::from)
}

fn amazon_games() -> Vec<Game> {
    let Some(local) = known_folder("LOCALAPPDATA") else {
        return Vec::new();
    };
    let db = local.join(r"Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite");
    sqlite::query(&db, amazon::QUERY, 3)
        .iter()
        .filter_map(|row| {
            let [id, title, dir] = row.as_slice() else {
                return None;
            };
            let fuel = read(&Path::new(dir.trim()).join("fuel.json"));
            amazon::game(id, title, dir, fuel.as_deref())
        })
        .collect()
}

fn itch_games() -> Vec<Game> {
    let Some(roaming) = known_folder("APPDATA") else {
        return Vec::new();
    };
    sqlite::query(&roaming.join(r"itch\db\butler.db"), itch::QUERY, 4)
        .iter()
        .filter_map(|row| {
            let [id, title, classification, verdict] = row.as_slice() else {
                return None;
            };
            itch::game(id, title, classification, verdict)
        })
        .collect()
}

fn heroic_games() -> Vec<Game> {
    let Some(heroic) = known_folder("APPDATA").map(|r| r.join("heroic")) else {
        return Vec::new();
    };
    if !heroic.is_dir() {
        return Vec::new();
    }
    let mut games = Vec::new();
    if let Some(text) = read(&heroic.join(heroic::EPIC_LIST)) {
        games.extend(heroic::epic_games(&text));
    }
    if let Some(text) = read(&heroic.join(heroic::GOG_LIST)) {
        for install in heroic::gog_installs(&text) {
            let info = read(Path::new(&install.info_file()));
            games.push(heroic::gog_game(&install, info.as_deref()));
        }
    }
    if let Some(text) = read(&heroic.join(heroic::AMAZON_LIST)) {
        for install in heroic::amazon_installs(&text) {
            let fuel = read(Path::new(&install.fuel_file()));
            games.push(heroic::amazon_game(&install, fuel.as_deref()));
        }
    }
    games
}

/// The Game Bar's games whose program still exists and is not inside a game
/// already found (the game behind a store's starter, or a store game the
/// Game Bar saw too). Two programs of one game folder give one game.
fn game_bar_games(library: &[Game]) -> Vec<Game> {
    let mut folders: Vec<NormPath> = library
        .iter()
        .filter(|g| g.install_dir.len() > 3)
        .map(|g| NormPath::new(&g.install_dir))
        .collect();
    let mut games = Vec::new();
    for entry in registry::subkeys(
        Hive::CurrentUser,
        gamebar::CHILDREN_KEY,
        &[gamebar::PATH_VALUE],
    ) {
        let Some(game) = entry.get(gamebar::PATH_VALUE).and_then(gamebar::game) else {
            continue;
        };
        let exe = NormPath::new(game.exe.as_deref().unwrap_or_default());
        let dir = NormPath::new(&game.install_dir);
        let known = folders
            .iter()
            .any(|f| exe.is_within(f) || f.is_within(&dir));
        if known || !game.exe.as_deref().is_some_and(|e| Path::new(e).is_file()) {
            continue;
        }
        folders.push(dir);
        games.push(game);
    }
    games
}
