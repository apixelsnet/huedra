//! Stores found through the registry: GOG, Ubisoft Connect, EA app and
//! Rockstar Games Launcher. `huedra-win` reads the keys; this module turns
//! their values into games.

use super::{Game, GameId, Store};
use crate::paths::display_name;

/// One registry key and its string values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistryEntry {
    /// The key's own name (the last part of its path).
    pub key: String,
    /// String values as (name, data).
    pub values: Vec<(String, String)>,
}

impl RegistryEntry {
    /// A non-empty value, compared without regard to case.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim())
            .filter(|v| !v.is_empty())
    }
}

/// Rockstar keys that are launchers, not games.
const ROCKSTAR_NOT_GAMES: &[&str] = &[
    "Launcher",
    "Rockstar Games Launcher",
    "Rockstar Games Social Club",
];

fn folder(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).replace('/', r"\")
}

/// `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`.
pub fn gog_game(entry: &RegistryEntry) -> Option<Game> {
    let dir = folder(entry.get("path")?);
    let id = entry.get("gameID").unwrap_or(&entry.key);
    Some(Game {
        id: GameId::new(Store::Gog, id),
        name: entry.get("gameName").unwrap_or(&entry.key).to_owned(),
        store: Store::Gog,
        exe: entry.get("exe").map(str::to_owned),
        install_dir: dir,
    })
}

/// `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>`. The key has
/// no name, so the folder name is used.
pub fn ubisoft_game(entry: &RegistryEntry) -> Option<Game> {
    let dir = folder(entry.get("InstallDir")?);
    Some(Game {
        id: GameId::new(Store::Ubisoft, &entry.key),
        name: display_name(&dir).to_owned(),
        store: Store::Ubisoft,
        install_dir: dir,
        exe: None,
    })
}

/// `HKLM\SOFTWARE\WOW6432Node\EA Games\<name>`.
pub fn ea_game(entry: &RegistryEntry) -> Option<Game> {
    let dir = folder(entry.get("Install Dir")?);
    Some(Game {
        id: GameId::new(Store::Ea, &entry.key),
        name: entry.get("DisplayName").unwrap_or(&entry.key).to_owned(),
        store: Store::Ea,
        install_dir: dir,
        exe: None,
    })
}

/// `HKLM\SOFTWARE\WOW6432Node\Rockstar Games\<name>`.
pub fn rockstar_game(entry: &RegistryEntry) -> Option<Game> {
    if ROCKSTAR_NOT_GAMES
        .iter()
        .any(|n| n.eq_ignore_ascii_case(&entry.key))
    {
        return None;
    }
    let dir = folder(entry.get("InstallFolder")?);
    Some(Game {
        id: GameId::new(Store::Rockstar, &entry.key),
        name: entry.key.clone(),
        store: Store::Rockstar,
        install_dir: dir,
        exe: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, values: &[(&str, &str)]) -> RegistryEntry {
        RegistryEntry {
            key: key.to_owned(),
            values: values
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn reads_each_store() {
        let gog = gog_game(&entry(
            "1207658924",
            &[
                ("gameName", "The Witcher 3"),
                ("path", r"D:\GOG\Witcher 3\"),
                ("gameID", "1207658924"),
            ],
        ))
        .expect("gog");
        assert_eq!(
            (gog.id.as_str(), gog.install_dir.as_str()),
            ("gog:1207658924", r"D:\GOG\Witcher 3"),
            "gog"
        );

        let ubi = ubisoft_game(&entry(
            "5271",
            &[("InstallDir", "D:/Ubisoft/Star Wars Outlaws/")],
        ))
        .expect("ubisoft");
        assert_eq!(ubi.name, "Star Wars Outlaws", "folder name");
        assert_eq!(ubi.install_dir, r"D:\Ubisoft\Star Wars Outlaws", "slashes");
        assert!(
            ubisoft_game(&entry("1081", &[("InstallDir", "")])).is_none(),
            "empty folder"
        );

        let ea = ea_game(&entry(
            "Battlefield 2042",
            &[("Install Dir", r"C:\EA\BF2042\")],
        ))
        .expect("ea");
        assert_eq!(ea.name, "Battlefield 2042", "key name");

        assert!(
            rockstar_game(&entry("Launcher", &[("InstallFolder", r"C:\R")])).is_none(),
            "launcher"
        );
        assert!(
            rockstar_game(&entry(
                "Red Dead Redemption 2",
                &[("InstallFolder", r"C:\RDR2")]
            ))
            .is_some(),
            "game"
        );
    }
}
