//! The settings window's size, position and maximized state, remembered in
//! `window.toml` (local app-data). Feature doc: `docs/features/08-settings-window.md`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::files;

/// Smallest size accepted from the file, in physical pixels.
const MIN_SIDE: u32 = 300;
const MAX_BYTES: u64 = 4096;

/// Where the window was when it closed, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Whether it was maximized.
    pub maximized: bool,
}

impl WindowGeometry {
    /// Reads the file; missing, damaged or implausible values give `None`.
    pub fn load(path: &Path) -> Option<Self> {
        let text = files::read_limited(path, MAX_BYTES).ok().flatten()?;
        let geometry: Self = toml::from_str(&text).ok()?;
        (geometry.width >= MIN_SIDE && geometry.height >= MIN_SIDE).then_some(geometry)
    }

    /// Writes the file atomically.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        files::write_atomic(path, text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_rejects_tiny() {
        let dir = std::env::temp_dir().join(format!("huedra-geometry-{}", std::process::id()));
        let path = dir.join("window.toml");
        let g = WindowGeometry {
            x: -10,
            y: 20,
            width: 1040,
            height: 680,
            maximized: true,
        };
        g.save(&path).expect("save");
        assert_eq!(WindowGeometry::load(&path), Some(g), "round trip");
        WindowGeometry { width: 10, ..g }.save(&path).expect("save");
        assert_eq!(WindowGeometry::load(&path), None, "too small");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
