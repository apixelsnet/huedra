//! Steam: `steamapps\libraryfolders.vdf` lists the libraries, and each
//! library's `appmanifest_<id>.acf` describes one installed app.

use super::vdf::{self, Value};
use super::{Game, GameId, Store};

/// Apps that are never games (Steamworks Common Redistributables).
const NOT_GAMES: &[&str] = &["228980"];
/// `StateFlags` bit for "fully installed".
const INSTALLED: u32 = 4;

/// Library folders listed in `libraryfolders.vdf`, including the Steam
/// folder itself. Handles the current format (blocks with a `path`) and the
/// older one (numbered keys whose value is the path).
pub fn library_folders(vdf_text: &str) -> Vec<String> {
    let Some(root) = vdf::parse(vdf_text) else {
        return Vec::new();
    };
    let Some(folders) = root.get("libraryfolders") else {
        return Vec::new();
    };
    folders
        .entries()
        .iter()
        .filter(|(key, _)| key.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|(_, value)| match value {
            Value::Map(_) => value.text("path").map(str::to_owned),
            Value::Text(path) => Some(path.clone()),
        })
        .collect()
}

/// The game described by an `appmanifest_*.acf`, if it is a fully installed
/// game. `library` is the library folder containing `steamapps`.
pub fn manifest_game(acf_text: &str, library: &str) -> Option<Game> {
    let root = vdf::parse(acf_text)?;
    let state = root.get("AppState")?;
    let app_id = state.text("appid")?.trim();
    let name = state.text("name")?.trim();
    let folder = state.text("installdir")?.trim();
    let flags: u32 = state.text("StateFlags")?.trim().parse().ok()?;
    if NOT_GAMES.contains(&app_id) || name.is_empty() || folder.is_empty() || flags & INSTALLED == 0
    {
        return None;
    }
    let library = library.trim_end_matches(['\\', '/']);
    Some(Game {
        id: GameId::new(Store::Steam, app_id),
        name: name.to_owned(),
        store: Store::Steam,
        install_dir: format!(r"{library}\steamapps\common\{folder}"),
        exe: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOLDERS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "215330008" }
	}
	"1"
	{
		"path"		"E:\\SteamLibrary"
	}
}"#;

    const MANIFEST: &str = r#"
"AppState"
{
	"appid"		"1145350"
	"name"		"Hades II"
	"StateFlags"		"6"
	"installdir"		"Hades II"
}"#;

    #[test]
    fn reads_libraries() {
        assert_eq!(
            library_folders(FOLDERS),
            vec![
                r"C:\Program Files (x86)\Steam".to_owned(),
                r"E:\SteamLibrary".to_owned()
            ],
            "both libraries"
        );
        let old = r#""LibraryFolders" { "TimeNextStatsReport" "1" "1" "D:\\Games" }"#;
        assert_eq!(
            library_folders(old),
            vec![r"D:\Games".to_owned()],
            "old format"
        );
    }

    #[test]
    fn reads_manifest() {
        let game = manifest_game(MANIFEST, r"E:\SteamLibrary\").expect("game");
        assert_eq!(game.id.as_str(), "steam:1145350", "id");
        assert_eq!(game.name, "Hades II", "name");
        assert_eq!(
            game.install_dir, r"E:\SteamLibrary\steamapps\common\Hades II",
            "folder"
        );
        let redist = MANIFEST.replace("1145350", "228980");
        assert!(manifest_game(&redist, "E:").is_none(), "redistributables");
        let downloading = MANIFEST.replace("\"6\"", "\"1026\"");
        assert!(manifest_game(&downloading, "E:").is_none(), "not installed");
    }
}
