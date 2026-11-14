//! A display mode a game asks for (resolution and refresh rate), and the
//! choice of the closest mode a display offers. Feature doc:
//! `docs/features/02-engine.md`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A resolution and refresh rate, such as 2560 × 1440 at 165 Hz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayMode {
    width: u32,
    height: u32,
    hz: u32,
}

/// The text is not a mode such as `2560x1440@165`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("\"{0}\" is not a display mode such as 2560x1440@165")]
pub struct ModeError(String);

impl DisplayMode {
    /// A mode; `None` when a part is zero.
    pub fn new(width: u32, height: u32, hz: u32) -> Option<Self> {
        (width > 0 && height > 0 && hz > 0).then_some(Self { width, height, hz })
    }

    /// Pixels across.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Pixels down.
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Refresh rate in hertz.
    pub const fn hz(self) -> u32 {
        self.hz
    }

    /// "2560 × 1440, 165 Hz", for the window.
    pub fn label(self) -> String {
        format!("{} × {}, {} Hz", self.width, self.height, self.hz)
    }

    /// The mode to use when `self` is asked for and the display offers
    /// `offered`: the same mode; else the same resolution at the highest
    /// refresh rate not above the one asked for; else the same resolution
    /// at its lowest rate. `None` when the display lacks the resolution.
    pub fn closest(self, offered: &[Self]) -> Option<Self> {
        if offered.contains(&self) {
            return Some(self);
        }
        let same = || {
            offered
                .iter()
                .filter(|m| m.width == self.width && m.height == self.height)
        };
        same()
            .filter(|m| m.hz <= self.hz)
            .max_by_key(|m| m.hz)
            .or_else(|| same().min_by_key(|m| m.hz))
            .copied()
    }
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}@{}", self.width, self.height, self.hz)
    }
}

impl FromStr for DisplayMode {
    type Err = ModeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || ModeError(text.to_owned());
        let (size, hz) = text.trim().split_once('@').ok_or_else(bad)?;
        let (w, h) = size.split_once(['x', 'X', '×']).ok_or_else(bad)?;
        let number = |s: &str| s.trim().parse::<u32>().map_err(|_| bad());
        let hz = hz.trim().trim_end_matches("Hz").trim_end_matches("hz");
        Self::new(number(w)?, number(h)?, number(hz)?).ok_or_else(bad)
    }
}

impl Serialize for DisplayMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for DisplayMode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// The modes to offer in a list: each resolution and rate once, the
/// largest first and, within a resolution, the fastest first.
pub fn list(mut modes: Vec<DisplayMode>) -> Vec<DisplayMode> {
    modes.sort_by(|a, b| b.cmp(a));
    modes.dedup();
    modes
}

/// The modes worth offering for a game on a display showing `current`:
/// the same shape of picture (aspect ratio within 1 %) at 50 Hz or more,
/// listed as by [`list`]. Video rates (24, 25, 30 Hz) and other shapes,
/// which games rarely want, would make the list hundreds long.
pub fn for_games(offered: Vec<DisplayMode>, current: Option<DisplayMode>) -> Vec<DisplayMode> {
    let Some(current) = current else {
        return list(offered);
    };
    let ratio = |m: &DisplayMode| f64::from(m.width) / f64::from(m.height);
    let wanted = ratio(&current);
    list(
        offered
            .into_iter()
            .filter(|m| m.hz >= 50 && (ratio(m) / wanted - 1.0).abs() < 0.01)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(w: u32, h: u32, hz: u32) -> DisplayMode {
        DisplayMode::new(w, h, hz).expect("valid")
    }

    #[test]
    fn parses_and_writes() {
        assert_eq!("2560x1440@165".parse(), Ok(m(2560, 1440, 165)), "plain");
        assert_eq!(
            " 1920 × 1080 @ 60Hz ".parse(),
            Ok(m(1920, 1080, 60)),
            "loose"
        );
        assert!("1920x1080".parse::<DisplayMode>().is_err(), "no rate");
        assert!("0x1080@60".parse::<DisplayMode>().is_err(), "zero");
        assert_eq!(m(3840, 2160, 144).to_string(), "3840x2160@144", "text");
        assert_eq!(m(3840, 2160, 144).label(), "3840 × 2160, 144 Hz", "label");
    }

    #[test]
    fn picks_the_closest_mode() {
        let offered = [
            m(2560, 1440, 165),
            m(2560, 1440, 144),
            m(2560, 1440, 60),
            m(1920, 1080, 240),
        ];
        assert_eq!(
            m(2560, 1440, 144).closest(&offered),
            Some(m(2560, 1440, 144)),
            "exact"
        );
        assert_eq!(
            m(2560, 1440, 240).closest(&offered),
            Some(m(2560, 1440, 165)),
            "slower when HDR leaves less bandwidth"
        );
        assert_eq!(
            m(2560, 1440, 50).closest(&offered),
            Some(m(2560, 1440, 60)),
            "lowest rate when none is slower"
        );
        assert_eq!(
            m(3840, 2160, 60).closest(&offered),
            None,
            "resolution missing"
        );
    }

    #[test]
    fn offers_games_the_same_shape_at_game_rates() {
        let offered = vec![
            m(2560, 1440, 360),
            m(2560, 1440, 24),
            m(1920, 1080, 60),
            m(1366, 768, 60),
            m(1920, 1200, 60),
            m(1024, 768, 60),
        ];
        assert_eq!(
            for_games(offered, Some(m(2560, 1440, 60))),
            vec![m(2560, 1440, 360), m(1920, 1080, 60), m(1366, 768, 60)],
            "16:9 at 50 Hz or more"
        );
    }

    #[test]
    fn lists_each_mode_once_largest_first() {
        let modes = list(vec![
            m(1920, 1080, 60),
            m(2560, 1440, 60),
            m(2560, 1440, 165),
            m(1920, 1080, 60),
        ]);
        assert_eq!(
            modes,
            vec![m(2560, 1440, 165), m(2560, 1440, 60), m(1920, 1080, 60)],
            "sorted, deduplicated"
        );
    }
}
