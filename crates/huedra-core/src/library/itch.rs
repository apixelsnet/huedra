//! itch.io app: its SQLite database `%AppData%\itch\db\butler.db` has one
//! row in `caves` per install, joined with `games`. An install's `verdict`
//! (JSON) gives its folder and the programs the app would start.
//! `huedra-win` runs the query; this module turns a row into a game.

use serde::Deserialize;

use super::{Game, GameId, Store, clean_folder, clean_name, is_helper_program};
use crate::paths::display_name;

/// The query whose rows [`game`] reads: id, title, classification, verdict.
pub const QUERY: &str = "SELECT games.id, games.title, games.classification, caves.verdict \
     FROM caves JOIN games ON games.id = caves.game_id";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Verdict {
    #[serde(default)]
    base_path: String,
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    #[serde(default)]
    path: String,
    #[serde(default)]
    flavor: String,
}

/// One install from [`QUERY`]. Only entries classified as games are kept
/// (the app also installs tools, comics, soundtracks and assets).
pub fn game(id: &str, title: &str, classification: &str, verdict: &str) -> Option<Game> {
    let name = clean_name(title);
    let id = id.trim();
    if id.is_empty() || name.is_empty() || classification != "game" {
        return None;
    }
    let verdict: Verdict = serde_json::from_str(verdict).ok()?;
    let dir = clean_folder(&verdict.base_path);
    if dir.is_empty() {
        return None;
    }
    let exe = verdict.candidates.iter().find_map(|c| {
        let path = c.path.trim().replace('/', r"\");
        let file = display_name(&path).to_lowercase();
        (c.flavor == "windows"
            && file.ends_with(".exe")
            && !path.contains("..")
            && !is_helper_program(&file))
        .then(|| format!(r"{dir}\{path}"))
    });
    Some(Game {
        id: GameId::new(Store::Itch, id),
        name,
        store: Store::Itch,
        install_dir: dir,
        exe,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_install() {
        let verdict = r#"{"basePath": "C:\\Users\\x\\AppData\\Roaming\\itch\\apps\\celeste",
            "totalSize": 1, "candidates": [
                {"path": "UnityCrashHandler64.exe", "flavor": "windows"},
                {"path": "Celeste.exe", "flavor": "windows", "arch": "386"}
            ]}"#;
        let found = game("90058", "Celeste", "game", verdict).expect("game");
        assert_eq!(found.id.as_str(), "itch:90058", "id");
        assert_eq!(
            found.exe.as_deref(),
            Some(r"C:\Users\x\AppData\Roaming\itch\apps\celeste\Celeste.exe"),
            "first Windows program that is not a helper"
        );
        assert!(
            game("1", "Soundtrack", "soundtrack", verdict).is_none(),
            "not a game"
        );
        assert!(game("1", "Broken", "game", "{").is_none(), "bad verdict");
        let script =
            r#"{"basePath": "C:\\G", "candidates": [{"path": "game.sh", "flavor": "script"}]}"#;
        let found = game("2", "Game", "game", script).expect("game without program");
        assert_eq!(found.exe, None, "no Windows program");
    }
}
