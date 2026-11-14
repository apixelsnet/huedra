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
    /// Huedra leaves it to Windows' own setting.
    #[default]
    Windows,
    /// On for this game's programs.
    On,
    /// Off for this game's programs.
    Off,
}

impl AutoHdr {
    /// The choices in the order the window lists them.
    pub const ALL: [Self; 3] = [Self::Windows, Self::On, Self::Off];

    /// Text shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Windows => "As set in Windows",
            Self::On => "On",
            Self::Off => "Off",
        }
    }
}

/// The program's value with Auto HDR set as chosen, every other pair kept
/// in its place. `None` means the value should not exist (nothing is left
/// in it). [`AutoHdr::Windows`] removes only Huedra's pair.
pub fn edit(current: Option<&str>, choice: AutoHdr) -> Option<String> {
    let mut pairs: Vec<(String, String)> = current
        .unwrap_or_default()
        .split(';')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), value.trim().to_owned()))
        })
        .collect();
    let wanted = match choice {
        AutoHdr::Windows => None,
        AutoHdr::On => Some(ON.to_string()),
        AutoHdr::Off => Some(OFF.to_string()),
    };
    match (
        pairs.iter().position(|(n, _)| n.eq_ignore_ascii_case(KEY)),
        wanted,
    ) {
        (Some(i), Some(value)) => pairs[i].1 = value,
        (Some(i), None) => {
            pairs.remove(i);
        }
        (None, Some(value)) => pairs.insert(0, (KEY.to_owned(), value)),
        (None, None) => {}
    }
    if pairs.is_empty() {
        return None;
    }
    Some(pairs.iter().map(|(n, v)| format!("{n}={v};")).collect())
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
