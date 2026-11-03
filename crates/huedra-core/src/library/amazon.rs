//! Amazon Games: the installed games are rows of the app's SQLite database
//! `%LocalAppData%\Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite`
//! (table `DbSet`), and each game folder's `fuel.json` names its program.
//! `huedra-win` runs the query; this module turns a row into a game.

use serde::Deserialize;

use super::{Game, GameId, Store, clean_folder, clean_name, is_helper_program};

/// The query whose rows [`game`] reads: id, title, folder.
pub const QUERY: &str = "SELECT Id, ProductTitle, InstallDirectory FROM DbSet WHERE Installed = 1";

/// One installed game from [`QUERY`]. `fuel` is the text of the folder's
/// `fuel.json`, when there is one.
pub fn game(id: &str, title: &str, dir: &str, fuel: Option<&str>) -> Option<Game> {
    let (id, name, dir) = (id.trim(), clean_name(title), clean_folder(dir));
    if id.is_empty() || name.is_empty() || dir.is_empty() {
        return None;
    }
    Some(Game {
        id: GameId::new(Store::Amazon, id),
        exe: fuel.and_then(|f| fuel_program(f, &dir)),
        name,
        store: Store::Amazon,
        install_dir: dir,
    })
}

#[derive(Deserialize)]
struct Fuel {
    #[serde(rename = "Main")]
    main: Main,
}

#[derive(Deserialize)]
struct Main {
    #[serde(rename = "Command", default)]
    command: String,
}

/// The program a `fuel.json` starts (`Main` › `Command`), inside `dir`.
/// Used by Amazon Games and by Heroic for its Amazon installs.
pub fn fuel_program(fuel: &str, dir: &str) -> Option<String> {
    let fuel: Fuel = serde_json::from_str(fuel).ok()?;
    let command = fuel.main.command.trim().replace('/', r"\");
    let file = command
        .rsplit('\\')
        .next()
        .unwrap_or(&command)
        .to_lowercase();
    let inside = !command.contains("..") && !command.contains(':') && !command.starts_with('\\');
    (inside && file.ends_with(".exe") && !is_helper_program(&file))
        .then(|| format!(r"{dir}\{command}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_row_and_fuel() {
        let fuel = r#"{"SchemaVersion": "2", "Main": {"Command": "Bin/Game.exe", "Args": []}}"#;
        let game = game(
            "amzn1.adg.product.0000",
            "Some Game™",
            "C:/Amazon Games/Library/Some Game/",
            Some(fuel),
        )
        .expect("game");
        assert_eq!(
            (game.id.as_str(), game.name.as_str()),
            ("amazon:amzn1.adg.product.0000", "Some Game"),
            "id and clean name"
        );
        assert_eq!(
            game.exe.as_deref(),
            Some(r"C:\Amazon Games\Library\Some Game\Bin\Game.exe"),
            "program from fuel.json"
        );
        assert!(
            fuel_program(r#"{"Main": {"Command": "..\\..\\x.exe"}}"#, r"C:\G").is_none(),
            "outside the folder"
        );
        assert!(
            fuel_program(r#"{"Main": {"Command": "launch.bat"}}"#, r"C:\G").is_none(),
            "not a program"
        );
        assert!(super::game("", "X", r"C:\X", None).is_none(), "no id");
    }
}
