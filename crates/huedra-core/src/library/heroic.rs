//! Heroic Games Launcher keeps its own install lists under `%AppData%\heroic`,
//! one per store it serves; games installed through it are not in the Epic,
//! GOG or Amazon apps' lists.
//!
//! - Epic: `legendaryConfig\legendary\installed.json`, an object keyed by app
//!   name.
//! - GOG: `gog_store\installed.json`, `{"installed": [...]}`; the title and
//!   program come from the `goggame-<id>.info` file in each game folder.
//! - Amazon: `nile_config\nile\installed.json`, an array; the program comes
//!   from the game folder's `fuel.json`.

use serde::Deserialize;
use serde_json::Value;

use super::{Game, GameId, Store, amazon, clean_folder, clean_name, is_helper_program};
use crate::paths::display_name;

/// The three lists, relative to `%AppData%\heroic`.
pub const EPIC_LIST: &str = r"legendaryConfig\legendary\installed.json";
/// See [`EPIC_LIST`].
pub const GOG_LIST: &str = r"gog_store\installed.json";
/// See [`EPIC_LIST`].
pub const AMAZON_LIST: &str = r"nile_config\nile\installed.json";

fn windows_build(platform: &str) -> bool {
    platform.is_empty() || platform.to_lowercase().starts_with("win")
}

#[derive(Deserialize)]
struct EpicInstall {
    title: String,
    install_path: String,
    #[serde(default)]
    executable: String,
    #[serde(default)]
    is_dlc: bool,
    #[serde(default)]
    platform: String,
}

/// The Windows games (not add-ons) in legendary's `installed.json`.
pub fn epic_games(json: &str) -> Vec<Game> {
    let Ok(Value::Object(apps)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    apps.into_iter()
        .filter_map(|(app, value)| {
            let install: EpicInstall = serde_json::from_value(value).ok()?;
            let name = clean_name(&install.title);
            let dir = clean_folder(&install.install_path);
            if install.is_dlc
                || !windows_build(&install.platform)
                || name.is_empty()
                || dir.is_empty()
            {
                return None;
            }
            let program = install.executable.trim().replace('/', r"\");
            let exe = (!program.is_empty() && !program.contains(".."))
                .then(|| format!(r"{dir}\{program}"));
            Some(Game {
                id: GameId::new(Store::Heroic, &format!("epic-{app}")),
                name,
                store: Store::Heroic,
                install_dir: dir,
                exe,
            })
        })
        .collect()
}

/// A GOG game installed through Heroic, before its `goggame-<id>.info` is
/// read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GogInstall {
    /// GOG's product id.
    pub id: String,
    /// Install folder.
    pub dir: String,
}

impl GogInstall {
    /// The info file in the game folder that names the game and its program.
    pub fn info_file(&self) -> String {
        format!(r"{}\goggame-{}.info", self.dir, self.id)
    }
}

#[derive(Deserialize)]
struct GogList {
    installed: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GogEntry {
    app_name: String,
    #[serde(rename = "install_path")]
    install_path: String,
    #[serde(default)]
    platform: String,
    #[serde(default, rename = "is_dlc")]
    is_dlc: bool,
}

/// The Windows games (not add-ons) in Heroic's GOG `installed.json`.
pub fn gog_installs(json: &str) -> Vec<GogInstall> {
    let Ok(list) = serde_json::from_str::<GogList>(json) else {
        return Vec::new();
    };
    list.installed
        .into_iter()
        .filter_map(|value| {
            let entry: GogEntry = serde_json::from_value(value).ok()?;
            let id = entry.app_name.trim().to_owned();
            let dir = clean_folder(&entry.install_path);
            let safe_id = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric());
            (safe_id && !dir.is_empty() && !entry.is_dlc && windows_build(&entry.platform))
                .then_some(GogInstall { id, dir })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GogInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    play_tasks: Vec<PlayTask>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayTask {
    #[serde(default)]
    is_primary: bool,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    path: String,
}

/// The game for a GOG install. `info` is the text of its
/// `goggame-<id>.info`; without it the folder name is the title and the
/// program is not known.
pub fn gog_game(install: &GogInstall, info: Option<&str>) -> Game {
    let info: Option<GogInfo> = info.and_then(|text| serde_json::from_str(text).ok());
    let exe = info.as_ref().and_then(|info| {
        let task = info
            .play_tasks
            .iter()
            .find(|t| t.is_primary && t.kind == "FileTask")?;
        let path = task.path.trim().replace('/', r"\");
        let file = display_name(&path).to_lowercase();
        (file.ends_with(".exe") && !path.contains("..") && !is_helper_program(&file))
            .then(|| format!(r"{}\{path}", install.dir))
    });
    let name = info
        .map(|info| clean_name(&info.name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| display_name(&install.dir).to_owned());
    Game {
        id: GameId::new(Store::Heroic, &format!("gog-{}", install.id)),
        name,
        store: Store::Heroic,
        install_dir: install.dir.clone(),
        exe,
    }
}

/// An Amazon game installed through Heroic, before its `fuel.json` is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmazonInstall {
    /// Amazon's product id.
    pub id: String,
    /// Install folder.
    pub dir: String,
}

impl AmazonInstall {
    /// The file in the game folder that names its program.
    pub fn fuel_file(&self) -> String {
        format!(r"{}\fuel.json", self.dir)
    }
}

#[derive(Deserialize)]
struct NileEntry {
    id: String,
    path: String,
}

/// The games in nile's `installed.json`.
pub fn amazon_installs(json: &str) -> Vec<AmazonInstall> {
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    list.into_iter()
        .filter_map(|value| {
            let entry: NileEntry = serde_json::from_value(value).ok()?;
            let id = entry.id.trim().to_owned();
            let dir = clean_folder(&entry.path);
            (!id.is_empty() && !dir.is_empty()).then_some(AmazonInstall { id, dir })
        })
        .collect()
}

/// The game for an Amazon install. nile keeps no title, and Heroic names the
/// folder after the game, so the folder name is the title.
pub fn amazon_game(install: &AmazonInstall, fuel: Option<&str>) -> Game {
    Game {
        id: GameId::new(Store::Heroic, &format!("amazon-{}", install.id)),
        name: display_name(&install.dir).to_owned(),
        store: Store::Heroic,
        exe: fuel.and_then(|f| amazon::fuel_program(f, &install.dir)),
        install_dir: install.dir.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_epic_list() {
        let json = r#"{
            "Fortnite": {"title": "Fortnite", "install_path": "D:\\Heroic\\Fortnite",
                "executable": "FortniteGame/Binaries/Win64/FortniteLauncher.exe",
                "is_dlc": false, "platform": "Windows"},
            "Addon": {"title": "Addon", "install_path": "D:\\Heroic\\Addon", "is_dlc": true},
            "MacOnly": {"title": "Mac", "install_path": "/Applications/Mac", "platform": "Mac"},
            "Broken": 3
        }"#;
        let games = epic_games(json);
        assert_eq!(games.len(), 1, "only the Windows game");
        assert_eq!(games[0].id.as_str(), "heroic:epic-fortnite", "id");
        assert_eq!(
            games[0].exe.as_deref(),
            Some(r"D:\Heroic\Fortnite\FortniteGame\Binaries\Win64\FortniteLauncher.exe"),
            "program"
        );
        assert!(epic_games("[]").is_empty(), "wrong shape");
    }

    #[test]
    fn reads_gog_list_and_info() {
        let json = r#"{"installed": [
            {"appName": "1207658924", "install_path": "D:\\GOG Games\\The Witcher 3", "platform": "windows", "is_dlc": false},
            {"appName": "../x", "install_path": "D:\\X", "platform": "windows"},
            {"appName": "1", "install_path": "/home/x", "platform": "linux"}
        ]}"#;
        let installs = gog_installs(json);
        assert_eq!(installs.len(), 1, "one Windows game with a safe id");
        assert_eq!(
            installs[0].info_file(),
            r"D:\GOG Games\The Witcher 3\goggame-1207658924.info",
            "info file"
        );
        let info = r#"{"name": "The Witcher 3: Wild Hunt", "playTasks": [
            {"isPrimary": false, "type": "FileTask", "path": "REDprelauncher.exe"},
            {"isPrimary": true, "type": "FileTask", "path": "bin/x64/witcher3.exe"}
        ]}"#;
        let game = gog_game(&installs[0], Some(info));
        assert_eq!(
            (game.id.as_str(), game.name.as_str()),
            ("heroic:gog-1207658924", "The Witcher 3: Wild Hunt"),
            "id and title"
        );
        assert_eq!(
            game.exe.as_deref(),
            Some(r"D:\GOG Games\The Witcher 3\bin\x64\witcher3.exe"),
            "primary task"
        );
        let bare = gog_game(&installs[0], None);
        assert_eq!(
            (bare.name.as_str(), bare.exe),
            ("The Witcher 3", None),
            "folder name"
        );
    }

    #[test]
    fn reads_amazon_list() {
        let installs = amazon_installs(
            r#"[{"id": "amzn1.adg.product.1", "version": "1", "path": "D:\\Heroic\\Some Game"}]"#,
        );
        assert_eq!(installs.len(), 1, "one game");
        let fuel = r#"{"Main": {"Command": "Game.exe"}}"#;
        let game = amazon_game(&installs[0], Some(fuel));
        assert_eq!(
            (game.name.as_str(), game.exe.as_deref()),
            ("Some Game", Some(r"D:\Heroic\Some Game\Game.exe")),
            "name and program"
        );
        assert!(amazon_installs("{}").is_empty(), "wrong shape");
    }
}
