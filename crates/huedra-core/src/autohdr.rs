//! Windows Auto HDR per program: the choice in the settings and the text
//! Windows keeps for each program under
//! `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`, a value named after
//! the program's path holding `Name=value;` pairs such as
//! `SwapEffectUpgradeEnable=1;AutoHDREnable=4147;AppStatus=4;`.
//! Feature doc: `docs/features/04-game-library.md`.

use serde::{Deserialize, Serialize};

/// The pair's name for Auto HDR.
pub const KEY: &str = "AutoHDREnable";
/// What Windows Settings › Display › Graphics writes when a program's Auto
/// HDR switch is turned on (seen on Windows 11 build 26300).
pub const ON: u32 = 4147;
/// What it writes when the switch is turned off.
pub const OFF: u32 = 48;

/// Auto HDR for one game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoHdr {
    /// The game follows Huedra's default for games (off unless turned on in
    /// Settings). Written as `windows`, the name 1.2 used for "as set in
    /// Windows", so older files keep working. Passed to [`edit`], it
    /// removes Huedra's pair.
    #[default]
    Windows,
    /// On for this game's programs.
    On,
    /// Off for this game's programs.
    Off,
}

impl AutoHdr {
    /// On or off for a game: its own choice, or `default_on` when it
    /// follows the default.
    pub fn is_on(self, default_on: bool) -> bool {
        match self {
            Self::Windows => default_on,
            Self::On => true,
            Self::Off => false,
        }
    }

    /// The choice to save when a game's switch is set to `on`: following
    /// the default when that is what the default gives.
    pub fn choose(on: bool, default_on: bool) -> Self {
        if on == default_on {
            Self::Windows
        } else {
            Self::of(on)
        }
    }

    /// The choice that turns it on or off.
    pub fn of(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }
}

/// The program's value with Auto HDR set as chosen, every other pair kept
/// in its place. `None` means the value should not exist (nothing is left
/// in it). [`AutoHdr::Windows`] removes only Huedra's pair.
pub fn edit(current: Option<&str>, choice: AutoHdr) -> Option<String> {
    let wanted = match choice {
        AutoHdr::Windows => None,
        AutoHdr::On => Some(ON.to_string()),
        AutoHdr::Off => Some(OFF.to_string()),
    };
    set(current, wanted.as_deref())
}

/// The program's value with the Auto HDR pair set to `value` as written,
/// or removed for `None`, every other pair kept in its place: used to put
/// back what Windows had before Huedra first changed it. `None` means the
/// value should not exist.
pub fn set(current: Option<&str>, value: Option<&str>) -> Option<String> {
    let mut pairs: Vec<(String, String)> = current
        .unwrap_or_default()
        .split(';')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), value.trim().to_owned()))
        })
        .collect();
    match (
        pairs.iter().position(|(n, _)| n.eq_ignore_ascii_case(KEY)),
        value,
    ) {
        (Some(i), Some(value)) => value.clone_into(&mut pairs[i].1),
        (Some(i), None) => {
            pairs.remove(i);
        }
        (None, Some(value)) => pairs.insert(0, (KEY.to_owned(), value.to_owned())),
        (None, None) => {}
    }
    if pairs.is_empty() {
        return None;
    }
    Some(pairs.iter().map(|(n, v)| format!("{n}={v};")).collect())
}

/// The Auto HDR pair's value in a program's value, as written, if it has
/// one.
pub fn value(current: &str) -> Option<String> {
    current.split(';').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case(KEY)
            .then(|| value.trim().to_owned())
    })
}

/// Whether a program's value turns Auto HDR on (`Some(true)`), off
/// (`Some(false)`), or leaves it to the global setting (`None`).
pub fn read(current: &str) -> Option<bool> {
    current.split(';').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if !name.trim().eq_ignore_ascii_case(KEY) {
            return None;
        }
        // Bit 0 is the switch; the other bits mark it as set per program.
        value.trim().parse::<u32>().ok().map(|v| v & 1 == 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_and_removes_only_its_own_pair() {
        assert_eq!(
            edit(Some("AppStatus=4;"), AutoHdr::On).as_deref(),
            Some("AutoHDREnable=4147;AppStatus=4;"),
            "added first, the rest kept"
        );
        assert_eq!(
            edit(
                Some("SwapEffectUpgradeEnable=1;AutoHDREnable=4147;AppStatus=4;"),
                AutoHdr::Off
            )
            .as_deref(),
            Some("SwapEffectUpgradeEnable=1;AutoHDREnable=48;AppStatus=4;"),
            "replaced in place"
        );
        assert_eq!(
            edit(Some("AutoHDREnable=48;AppStatus=4;"), AutoHdr::Windows).as_deref(),
            Some("AppStatus=4;"),
            "removed"
        );
        assert_eq!(
            edit(Some("AutoHDREnable=4147;"), AutoHdr::Windows),
            None,
            "nothing left: no value"
        );
        assert_eq!(edit(None, AutoHdr::Windows), None, "nothing to do");
        assert_eq!(
            edit(None, AutoHdr::On).as_deref(),
            Some("AutoHDREnable=4147;"),
            "new value"
        );
    }

    #[test]
    fn follows_the_default_and_puts_back() {
        assert!(!AutoHdr::Windows.is_on(false), "default off");
        assert!(AutoHdr::Windows.is_on(true), "default on");
        assert!(AutoHdr::On.is_on(false), "own choice wins");
        assert_eq!(
            AutoHdr::choose(false, false),
            AutoHdr::Windows,
            "same as the default"
        );
        assert_eq!(AutoHdr::choose(true, false), AutoHdr::On, "differs");
        assert_eq!(AutoHdr::choose(false, true), AutoHdr::Off, "differs");
        let current = "SwapEffectUpgradeEnable=1;AutoHDREnable=48;";
        assert_eq!(value(current).as_deref(), Some("48"), "read as written");
        assert_eq!(value("AppStatus=4;"), None, "none");
        assert_eq!(
            set(Some(current), Some("4147")).as_deref(),
            Some("SwapEffectUpgradeEnable=1;AutoHDREnable=4147;"),
            "put back"
        );
        assert_eq!(
            set(Some(current), None).as_deref(),
            Some("SwapEffectUpgradeEnable=1;"),
            "removed when there was none"
        );
    }

    #[test]
    fn reads_the_switch() {
        assert_eq!(
            read("SwapEffectUpgradeEnable=1;AutoHDREnable=4147;"),
            Some(true),
            "on"
        );
        assert_eq!(read("AutoHDREnable=48;AppStatus=4;"), Some(false), "off");
        assert_eq!(read("AppStatus=4;"), None, "not set");
    }
}
