//! Platform-independent core of Huedra: rules, the display state engine,
//! configuration and file parsers.
//!
//! Nothing here calls Windows, so every test in this crate runs on Linux.
//! Feature docs: `docs/features/02-engine.md`, `04-game-library.md`,
//! `09-cli-ipc.md`, `10-config.md`, `13-game-info.md`.

#![forbid(unsafe_code)]

pub mod clock;
pub mod command;
pub mod config;
pub mod display;
pub mod engine;
pub mod files;
pub mod gameinfo;
pub mod geometry;
pub mod icon;
pub mod ipc;
pub mod library;
pub mod locations;
pub mod logfile;
pub mod paths;
pub mod tracker;

/// Application name as shown to users.
pub const APP_NAME: &str = "Huedra";

/// Version of this build, from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_three_numbers() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION should have three parts: {VERSION}");
        assert!(
            parts.iter().all(|part| part.parse::<u32>().is_ok()),
            "every part of VERSION should be a number: {VERSION}"
        );
    }
}
