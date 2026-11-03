//! Stores found through the registry: GOG, Ubisoft Connect, EA app,
//! Rockstar Games Launcher, Battle.net, Riot Games and HoYoPlay. `huedra-win`
//! reads the keys; this module turns their values into games.

use super::{Game, GameId, Store, clean_folder, clean_name};
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

/// Values read from each `...\CurrentVersion\Uninstall\<key>` entry for
/// [`battlenet_game`] and [`riot_game`].
pub const UNINSTALL_VALUES: &[&str] = &["DisplayName", "InstallLocation", "UninstallString"];

/// A Battle.net game: an uninstall entry whose uninstaller is Battle.net's,
/// with `--uid=<product>`. The Battle.net app itself, public test realms and
/// betas are skipped.
pub fn battlenet_game(entry: &RegistryEntry) -> Option<Game> {
    let uninstall = entry.get("UninstallString")?;
    if !uninstall.to_lowercase().contains("battle.net") {
        return None;
    }
    let uid = uninstall
        .split_once("--uid=")?
        .1
        .split(|c: char| c.is_whitespace() || c == '"')
        .next()
        .filter(|uid| !uid.is_empty())?;
    let name = clean_name(entry.get("DisplayName")?);
    let lower = name.to_lowercase();
    let test_build = lower.ends_with(" test") || lower.ends_with(" beta") || name.contains("PTR");
    let dir = clean_folder(entry.get("InstallLocation")?);
    if uid.eq_ignore_ascii_case("battle.net")
        || display_name(&dir).eq_ignore_ascii_case("Battle.net")
        || test_build
        || name.is_empty()
        || dir.is_empty()
    {
        return None;
    }
    Some(Game {
        id: GameId::new(Store::BattleNet, uid),
        name,
        store: Store::BattleNet,
        install_dir: dir,
        exe: None,
    })
}

/// A Riot Games game: the current user's uninstall entry
/// `Riot Game <product>.<patchline>`. The Riot Client and patchlines other
/// than `live` (the PBE test servers) are skipped. League of Legends is
/// narrowed to its `Game` folder, so that its client, which stays open
/// between matches, does not count as the game.
pub fn riot_game(entry: &RegistryEntry) -> Option<Game> {
    let prefix = entry
        .key
        .get(..10)
        .filter(|p| p.eq_ignore_ascii_case("Riot Game "))?;
    let product = entry.key[prefix.len()..].trim().to_lowercase();
    let (product, patchline) = product.rsplit_once('.').unwrap_or((product.as_str(), ""));
    if product.is_empty()
        || product.starts_with("riot_client")
        || !(patchline.is_empty() || patchline == "live")
    {
        return None;
    }
    let mut dir = clean_folder(entry.get("InstallLocation")?);
    if dir.is_empty() {
        return None;
    }
    if product == "league_of_legends" {
        dir.push_str(r"\Game");
    }
    Some(Game {
        id: GameId::new(Store::Riot, product),
        name: clean_name(entry.get("DisplayName").unwrap_or(product)),
        store: Store::Riot,
        install_dir: dir,
        exe: None,
    })
}

/// HoYoPlay keys, one subkey per game:
/// `HKCU\Software\Cognosphere\HYP\1_0` (global) and
/// `HKCU\Software\miHoYo\HYP\1_0` (mainland China).
pub const HOYOPLAY_KEYS: &[&str] = &[r"Software\Cognosphere\HYP\1_0", r"Software\miHoYo\HYP\1_0"];

/// Values read from each [`HOYOPLAY_KEYS`] subkey for [`hoyoplay_game`].
pub const HOYOPLAY_VALUES: &[&str] = &["GameInstallPath", "GameBiz"];

/// A HoYoPlay game. The key names the game by its internal code
/// (`hk4e_global`); the known ones get their titles, others the folder name.
/// Games listed but not installed have an empty folder and are skipped.
pub fn hoyoplay_game(entry: &RegistryEntry) -> Option<Game> {
    let dir = clean_folder(entry.get("GameInstallPath")?);
    if dir.is_empty() {
        return None;
    }
    let biz = entry.get("GameBiz").unwrap_or(&entry.key);
    let name = match biz.split('_').next().unwrap_or(biz) {
        "hk4e" => "Genshin Impact",
        "hkrpg" => "Honkai: Star Rail",
        "nap" => "Zenless Zone Zero",
        "bh3" => "Honkai Impact 3rd",
        _ => display_name(&dir),
    }
    .to_owned();
    Some(Game {
        id: GameId::new(Store::HoYoPlay, biz),
        name,
        store: Store::HoYoPlay,
        install_dir: dir,
        exe: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_launchers_from_uninstall_entries() {
        let bnet = |name: &str, uid: &str, dir: &str| {
            let uninstall = format!(
                r#""C:\ProgramData\Battle.net\Agent\Blizzard Uninstaller.exe" --lang=enUS --uid={uid} --displayname="{name}""#
            );
            battlenet_game(&entry(
                name,
                &[
                    ("DisplayName", name),
                    ("InstallLocation", dir),
                    ("UninstallString", &uninstall),
                ],
            ))
        };
        let diablo = bnet("Diablo IV", "fenris", r"D:\Games\Diablo IV\").expect("game");
        assert_eq!(
            (diablo.id.as_str(), diablo.install_dir.as_str()),
            ("battlenet:fenris", r"D:\Games\Diablo IV"),
            "battle.net game"
        );
        assert!(
            bnet(
                "Battle.net",
                "battle.net",
                r"C:\Program Files (x86)\Battle.net"
            )
            .is_none(),
            "the app itself"
        );
        assert!(
            bnet("Overwatch Test", "prometheus_test", r"D:\OW Test").is_none(),
            "test realm"
        );
        assert!(
            battlenet_game(&entry(
                "Other",
                &[
                    ("DisplayName", "Other"),
                    ("InstallLocation", r"C:\Other"),
                    ("UninstallString", r"C:\Other\unins000.exe --uid=x"),
                ],
            ))
            .is_none(),
            "not battle.net"
        );

        let riot = |key: &str, name: &str, dir: &str| {
            riot_game(&entry(
                key,
                &[("DisplayName", name), ("InstallLocation", dir)],
            ))
        };
        let valorant = riot(
            "Riot Game valorant.live",
            "VALORANT",
            "E:/Games/Riot Games/VALORANT/live",
        )
        .expect("game");
        assert_eq!(
            (valorant.id.as_str(), valorant.install_dir.as_str()),
            ("riot:valorant", r"E:\Games\Riot Games\VALORANT\live"),
            "riot game"
        );
        let league = riot(
            "Riot Game league_of_legends.live",
            "League of Legends",
            "E:/Games/Riot Games/League of Legends",
        )
        .expect("league");
        assert_eq!(
            league.install_dir, r"E:\Games\Riot Games\League of Legends\Game",
            "league without its client"
        );
        assert!(
            riot(
                "Riot Game league_of_legends.pbe",
                "League PBE",
                "E:/LoL PBE"
            )
            .is_none(),
            "test patchline"
        );
        assert!(
            riot("Riot Game Riot_Client.", "Riot Client ", "E:/Riot Client/").is_none(),
            "the client itself"
        );
        assert!(
            riot("Steam App 1", "Game", "C:/Game").is_none(),
            "other uninstall entry"
        );
    }

    #[test]
    fn reads_hoyoplay() {
        let genshin = hoyoplay_game(&entry(
            "hk4e_global",
            &[
                ("GameInstallPath", r"E:\Games\Genshin Impact game"),
                ("GameBiz", "hk4e_global"),
            ],
        ))
        .expect("game");
        assert_eq!(
            (genshin.id.as_str(), genshin.name.as_str()),
            ("hoyoplay:hk4e_global", "Genshin Impact"),
            "known title"
        );
        let other = hoyoplay_game(&entry(
            "abc_global",
            &[("GameInstallPath", r"E:\New Game\")],
        ))
        .expect("other");
        assert_eq!(other.name, "New Game", "folder name");
        assert!(
            hoyoplay_game(&entry("hkrpg_global", &[("GameInstallPath", "")])).is_none(),
            "not installed"
        );
    }

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
