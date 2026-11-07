//! Portrait art for the Games page's cover grid: the 600 x 900 library
//! image Steam keeps on this PC for each game it has shown. Nothing is
//! downloaded; games of other stores keep their icon on a tinted tile.
//! Loaded only once the grid is first shown, and shrunk to 200 x 300 so a
//! large library stays light. Feature doc: `docs/features/08-settings-window.md`.

use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::thread;

use huedra_core::icons;
use huedra_core::library::{Game, GameId};
use huedra_win::stores;

/// Longest side accepted from a cover file.
const MAX_SIDE: usize = 1200;

/// A decoded cover.
pub struct Cover {
    /// The game.
    pub id: GameId,
    /// Straight RGBA pixels.
    pub rgba: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Loads the covers of `games` on a thread of its own, handing each to
/// `deliver` as it is ready.
pub fn spawn(games: Vec<Game>, deliver: impl Fn(Cover) + Send + 'static) {
    let _ = thread::Builder::new()
        .name("huedra-covers".into())
        .spawn(move || {
            let Some(root) = stores::steam_root() else {
                return;
            };
            for game in games {
                let Some(app_id) = game.id.steam_app_id() else {
                    continue;
                };
                if let Some(path) = icons::steam_cached_cover(&root, app_id)
                    && let Some(cover) = load(game.id, &path)
                {
                    deliver(cover);
                }
            }
        });
}

fn load(id: GameId, path: &Path) -> Option<Cover> {
    if fs::metadata(path).ok()?.len() > icons::MAX_COVER_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let options = zune_jpeg::zune_core::options::DecoderOptions::default()
        .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA)
        .set_max_width(MAX_SIDE)
        .set_max_height(MAX_SIDE);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), options);
    let rgba = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    // 600 x 900 becomes 200 x 300; smaller art is kept as it is.
    let factor = (width / 200).max(1);
    let (rgba, width, height) = icons::shrink(&rgba, width, height, factor)?;
    Some(Cover {
        id,
        rgba,
        width,
        height,
    })
}
