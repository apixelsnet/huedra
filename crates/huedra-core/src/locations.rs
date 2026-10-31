//! Where Huedra keeps its files. Normally the roaming and local app-data
//! folders; in portable mode (a file named `portable` next to the program) a
//! `data` folder beside it. Feature doc: `docs/features/10-config.md`.

use std::env;
use std::path::{Path, PathBuf};

/// Every file location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locations {
    /// `config.toml` (roaming).
    pub config: PathBuf,
    /// `owned.toml`, the recovery record (local).
    pub owned: PathBuf,
    /// `huedra.log` (local).
    pub log: PathBuf,
    /// `gameinfo.toml`, the PCGamingWiki cache (local).
    pub game_info: PathBuf,
    /// `window.toml`, the settings window's size and position (local).
    pub window: PathBuf,
    /// `icons\`, the game icons and their index (local).
    pub icons: PathBuf,
    /// Whether portable mode is on.
    pub portable: bool,
}

/// The folders could not be found.
#[derive(Debug, thiserror::Error)]
pub enum LocationError {
    /// The program's own path is unknown.
    #[error("could not find the program's location")]
    NoExe(#[source] std::io::Error),
    /// A required environment variable is missing.
    #[error("the environment variable {0} is not set")]
    NoVariable(&'static str),
}

impl Locations {
    /// Finds the folders for the running program.
    ///
    /// # Errors
    /// [`LocationError`] when the program path or app-data folders are unknown.
    pub fn resolve() -> Result<Self, LocationError> {
        let exe = env::current_exe().map_err(LocationError::NoExe)?;
        let dir = exe.parent().unwrap_or(Path::new("."));
        if dir.join("portable").is_file() {
            return Ok(Self::in_folders(&dir.join("data"), &dir.join("data"), true));
        }
        let folder = |var: &'static str| -> Result<PathBuf, LocationError> {
            let base = env::var_os(var).ok_or(LocationError::NoVariable(var))?;
            Ok(PathBuf::from(base).join("Huedra"))
        };
        Ok(Self::in_folders(
            &folder("APPDATA")?,
            &folder("LOCALAPPDATA")?,
            false,
        ))
    }

    /// Locations inside the given roaming and local folders.
    pub fn in_folders(roaming: &Path, local: &Path, portable: bool) -> Self {
        Self {
            config: roaming.join("config.toml"),
            owned: local.join("owned.toml"),
            log: local.join("huedra.log"),
            game_info: local.join("gameinfo.toml"),
            window: local.join("window.toml"),
            icons: local.join("icons"),
            portable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_go_where_expected() {
        let l = Locations::in_folders(Path::new("roam"), Path::new("local"), false);
        assert_eq!(
            l.config,
            Path::new("roam").join("config.toml"),
            "config roams"
        );
        assert_eq!(
            l.owned,
            Path::new("local").join("owned.toml"),
            "record is local"
        );
        assert_eq!(
            l.game_info,
            Path::new("local").join("gameinfo.toml"),
            "cache is local"
        );
    }
}
