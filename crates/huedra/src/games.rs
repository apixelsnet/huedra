//! `huedra games`: lists the games Huedra found and what it does for each.
//! Feature doc: `docs/features/04-game-library.md`.

use std::time::Instant;

use huedra_core::config::{Config, GameHdr};
use huedra_core::library;
use huedra_core::locations::Locations;
use huedra_win::stores;

/// Scans every store and prints the result.
pub fn run() -> anyhow::Result<()> {
    let locations = Locations::resolve()?;
    let config = Config::load(&locations.config).unwrap_or_default();
    let started = Instant::now();
    let mut games = config.manual_games();
    games.extend(stores::scan());
    let elapsed = started.elapsed();
    let mut games = library::without_repeats(games);
    games.sort_by_key(|g| g.name.to_lowercase());
    let width = games
        .iter()
        .map(|g| g.name.chars().count())
        .max()
        .unwrap_or(0);
    for game in &games {
        let hdr = config
            .game(&game.id)
            .map(|s| s.hdr)
            .unwrap_or(GameHdr::EnableWhileRunning);
        println!(
            "{:width$}  {:14}  {:20}  {}",
            game.name,
            game.store.label(),
            hdr.label(),
            game.install_dir
        );
    }
    println!(
        "\n{} games found in {} ms.",
        games.len(),
        elapsed.as_millis()
    );
    Ok(())
}
