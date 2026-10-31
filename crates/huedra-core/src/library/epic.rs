//! Epic Games Store: one JSON manifest (`*.item`) per installed app in
//! `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests`.

use serde::Deserialize;

use super::{Game, GameId, Store};

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "InstallLocation")]
    install_location: String,
    #[serde(rename = "LaunchExecutable", default)]
    launch_executable: String,
    #[serde(rename = "AppName")]
    app_name: String,
    #[serde(rename = "AppCategories", default)]
    app_categories: Vec<String>,
    #[serde(rename = "bIsIncompleteInstall", default)]
    incomplete: bool,
}

/// The game a manifest describes, if it is a complete game installation.
pub fn item_game(json: &str) -> Option<Game> {
    let item: Item = serde_json::from_str(json).ok()?;
    let is_game =
        item.app_categories.is_empty() || item.app_categories.iter().any(|c| c == "games");
    if item.incomplete || !is_game || item.install_location.trim().is_empty() {
        return None;
    }
    let dir = item
        .install_location
        .trim_end_matches(['\\', '/'])
        .to_owned();
    let exe = (!item.launch_executable.is_empty())
        .then(|| format!(r"{dir}\{}", item.launch_executable.replace('/', r"\")));
    Some(Game {
        id: GameId::new(Store::Epic, &item.app_name),
        name: item.display_name,
        store: Store::Epic,
        install_dir: dir,
        exe,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_item() {
        let json = r#"{
            "DisplayName": "Alan Wake 2",
            "InstallLocation": "D:\\Epic\\AlanWake2",
            "LaunchExecutable": "AlanWake2.exe",
            "AppName": "Dill",
            "AppCategories": ["public", "games", "applications"],
            "bIsIncompleteInstall": false
        }"#;
        let game = item_game(json).expect("game");
        assert_eq!(game.id.as_str(), "epic:dill", "id");
        assert_eq!(
            game.exe.as_deref(),
            Some(r"D:\Epic\AlanWake2\AlanWake2.exe"),
            "exe"
        );
        let engine = json.replace(r#""games", "#, "");
        assert!(item_game(&engine).is_none(), "not a game");
        let partial = json.replace(
            "\"bIsIncompleteInstall\": false",
            "\"bIsIncompleteInstall\": true",
        );
        assert!(item_game(&partial).is_none(), "incomplete");
        assert!(item_game("{").is_none(), "malformed");
    }
}
