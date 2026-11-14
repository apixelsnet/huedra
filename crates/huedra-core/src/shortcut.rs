//! Keyboard shortcuts as written in the settings ("Win+Alt+H"): parsing,
//! checking and the canonical text. Feature doc: `docs/features/07-tray-hotkeys.md`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The modifier keys of a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    /// The Windows key.
    pub const WIN: Self = Self(1);
    /// Ctrl.
    pub const CTRL: Self = Self(2);
    /// Alt.
    pub const ALT: Self = Self(4);
    /// Shift.
    pub const SHIFT: Self = Self(8);

    /// Whether every key of `other` is held in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Both sets of keys.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The key pressed with the modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A to Z (the ASCII capital).
    Letter(u8),
    /// 0 to 9 on the main keyboard (the ASCII digit).
    Digit(u8),
    /// F1 to F24.
    Function(u8),
    /// A key with a name.
    Named(NamedKey),
}

/// Keys with names that a shortcut may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedKey {
    /// Insert.
    Insert,
    /// Delete.
    Delete,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Pause.
    Pause,
    /// Scroll Lock.
    ScrollLock,
}

impl NamedKey {
    const ALL: [Self; 8] = [
        Self::Insert,
        Self::Delete,
        Self::Home,
        Self::End,
        Self::PageUp,
        Self::PageDown,
        Self::Pause,
        Self::ScrollLock,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Insert => "Insert",
            Self::Delete => "Delete",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
            Self::Pause => "Pause",
            Self::ScrollLock => "ScrollLock",
        }
    }

    fn virtual_key(self) -> u32 {
        match self {
            Self::Insert => 0x2D,
            Self::Delete => 0x2E,
            Self::Home => 0x24,
            Self::End => 0x23,
            Self::PageUp => 0x21,
            Self::PageDown => 0x22,
            Self::Pause => 0x13,
            Self::ScrollLock => 0x91,
        }
    }
}

impl Key {
    /// The Windows virtual-key code.
    pub fn virtual_key(self) -> u32 {
        match self {
            Self::Letter(c) | Self::Digit(c) => u32::from(c),
            Self::Function(n) => 0x70 + u32::from(n) - 1,
            Self::Named(k) => k.virtual_key(),
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let upper = text.to_ascii_uppercase();
        match upper.as_bytes() {
            [c @ b'A'..=b'Z'] => return Some(Self::Letter(*c)),
            [c @ b'0'..=b'9'] => return Some(Self::Digit(*c)),
            _ => {}
        }
        if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
            && (1..=24).contains(&n)
        {
            return Some(Self::Function(n));
        }
        let squashed: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
        let squashed = match squashed.as_str() {
            "INS" => "INSERT",
            "DEL" => "DELETE",
            "PGUP" => "PAGEUP",
            "PGDN" => "PAGEDOWN",
            "BREAK" => "PAUSE",
            other => other,
        };
        NamedKey::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(squashed))
            .map(Self::Named)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Letter(c) | Self::Digit(c) => write!(f, "{}", char::from(*c)),
            Self::Function(n) => write!(f, "F{n}"),
            Self::Named(k) => f.write_str(k.name()),
        }
    }
}

/// A shortcut such as Win+Alt+H.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    modifiers: Modifiers,
    key: Key,
}

/// Why a shortcut's text was not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    /// No key was named.
    #[error("Name a key, for example Win+Alt+H.")]
    NoKey,
    /// A part is not a key Huedra knows.
    #[error("\"{0}\" is not a key Huedra can use.")]
    UnknownKey(String),
    /// More than one key besides the modifiers.
    #[error("Use one key with Win, Ctrl, Alt or Shift, for example Win+Alt+H.")]
    TwoKeys,
    /// A shortcut needs Win, Ctrl or Alt, so typing never triggers it.
    #[error("Add Win, Ctrl or Alt, so typing doesn't set it off.")]
    NoModifier,
    /// Windows uses this one.
    #[error("Windows uses {0} itself.")]
    Reserved(String),
}

impl Shortcut {
    /// A shortcut from its parts; used for the defaults.
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// The modifier keys.
    pub const fn modifiers(self) -> Modifiers {
        self.modifiers
    }

    /// The key.
    pub const fn key(self) -> Key {
        self.key
    }

    /// Shortcuts Windows keeps for itself, which `RegisterHotKey` would
    /// refuse or take from Windows: Win+Alt+B (HDR), Win+L,
    /// Ctrl+Alt+Delete.
    fn is_reserved(self) -> bool {
        let only = |m: Modifiers| self.modifiers == m;
        match self.key {
            Key::Letter(b'B') => only(Modifiers::WIN.with(Modifiers::ALT)),
            Key::Letter(b'L') => only(Modifiers::WIN),
            Key::Named(NamedKey::Delete) => only(Modifiers::CTRL.with(Modifiers::ALT)),
            _ => false,
        }
    }
}

impl FromStr for Shortcut {
    type Err = ShortcutError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut modifiers = Modifiers::default();
        let mut key = None;
        for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            let modifier = match part.to_ascii_lowercase().as_str() {
                "win" | "windows" | "meta" | "super" => Some(Modifiers::WIN),
                "ctrl" | "control" => Some(Modifiers::CTRL),
                "alt" => Some(Modifiers::ALT),
                "shift" => Some(Modifiers::SHIFT),
                _ => None,
            };
            if let Some(m) = modifier {
                modifiers = modifiers.with(m);
                continue;
            }
            let parsed =
                Key::parse(part).ok_or_else(|| ShortcutError::UnknownKey(part.to_owned()))?;
            if key.replace(parsed).is_some() {
                return Err(ShortcutError::TwoKeys);
            }
        }
        let key = key.ok_or(ShortcutError::NoKey)?;
        let strong = modifiers.contains(Modifiers::WIN)
            || modifiers.contains(Modifiers::CTRL)
            || modifiers.contains(Modifiers::ALT);
        let spare_function = matches!(key, Key::Function(13..=24));
        if !strong && !spare_function {
            return Err(ShortcutError::NoModifier);
        }
        let shortcut = Self { modifiers, key };
        if shortcut.is_reserved() {
            return Err(ShortcutError::Reserved(shortcut.to_string()));
        }
        Ok(shortcut)
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (m, name) in [
            (Modifiers::WIN, "Win"),
            (Modifiers::CTRL, "Ctrl"),
            (Modifiers::ALT, "Alt"),
            (Modifiers::SHIFT, "Shift"),
        ] {
            if self.modifiers.contains(m) {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", self.key)
    }
}

/// One shortcut setting in the file: keys, off (an empty text), or text
/// that is not a valid shortcut, kept as written so saving the file does
/// not lose it; the default applies meanwhile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// These keys.
    Keys(Shortcut),
    /// No shortcut.
    Off,
    /// Not valid; the default applies.
    Invalid(String),
}

impl Binding {
    /// From the text in the settings window or the file; empty text turns
    /// the shortcut off.
    ///
    /// # Errors
    /// [`ShortcutError`] when the text is not a shortcut Huedra can use.
    pub fn parse(text: &str) -> Result<Self, ShortcutError> {
        if text.trim().is_empty() {
            return Ok(Self::Off);
        }
        text.parse().map(Self::Keys)
    }

    /// The keys in effect: invalid text falls back to `default`.
    pub fn resolve(&self, default: Shortcut) -> Option<Shortcut> {
        match self {
            Self::Keys(s) => Some(*s),
            Self::Off => None,
            Self::Invalid(_) => Some(default),
        }
    }

    /// The text written to the file and shown in the window.
    pub fn text(&self) -> String {
        match self {
            Self::Keys(s) => s.to_string(),
            Self::Off => String::new(),
            Self::Invalid(t) => t.clone(),
        }
    }
}

impl Serialize for Binding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text())
    }
}

impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(Self::parse(&text).unwrap_or(Self::Invalid(text)))
    }
}

/// The three shortcuts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    /// Turn HDR on or off.
    pub toggle_hdr: Binding,
    /// Change the mode.
    pub cycle_mode: Binding,
    /// Pause or resume.
    pub pause: Binding,
}

const WIN_ALT: Modifiers = Modifiers(Modifiers::WIN.0 | Modifiers::ALT.0);
/// Win+Alt+H.
pub const DEFAULT_TOGGLE_HDR: Shortcut = Shortcut::new(WIN_ALT, Key::Letter(b'H'));
/// Win+Alt+O.
pub const DEFAULT_CYCLE_MODE: Shortcut = Shortcut::new(WIN_ALT, Key::Letter(b'O'));
/// Win+Alt+P.
pub const DEFAULT_PAUSE: Shortcut = Shortcut::new(WIN_ALT, Key::Letter(b'P'));

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle_hdr: Binding::Keys(DEFAULT_TOGGLE_HDR),
            cycle_mode: Binding::Keys(DEFAULT_CYCLE_MODE),
            pause: Binding::Keys(DEFAULT_PAUSE),
        }
    }
}

impl Shortcuts {
    /// Whether every shortcut has its default keys.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The keys in effect for turning HDR on or off, changing the mode and
    /// pausing, defaults standing in for invalid text; `None` for a
    /// shortcut turned off.
    pub fn resolved(&self) -> [Option<Shortcut>; 3] {
        [
            self.toggle_hdr.resolve(DEFAULT_TOGGLE_HDR),
            self.cycle_mode.resolve(DEFAULT_CYCLE_MODE),
            self.pause.resolve(DEFAULT_PAUSE),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_canonical_text() {
        let s: Shortcut = " alt + win + h ".parse().expect("valid");
        assert_eq!(s.to_string(), "Win+Alt+H", "canonical order and case");
        assert_eq!(s, DEFAULT_TOGGLE_HDR, "same as the default");
        let s: Shortcut = "Ctrl+Shift+F9".parse().expect("valid");
        assert_eq!(s.key().virtual_key(), 0x78, "F9 is VK 0x78");
        assert_eq!(s.to_string(), "Ctrl+Shift+F9", "function key text");
        let s: Shortcut = "ctrl+alt+pgdn".parse().expect("valid");
        assert_eq!(s.to_string(), "Ctrl+Alt+PageDown", "named key alias");
        let s: Shortcut = "F13".parse().expect("F13 to F24 need no modifier");
        assert_eq!(s.key().virtual_key(), 0x7C, "F13 is VK 0x7C");
    }

    #[test]
    fn refuses_bad_shortcuts() {
        assert_eq!("".parse::<Shortcut>(), Err(ShortcutError::NoKey), "empty");
        assert_eq!(
            "Win+Alt".parse::<Shortcut>(),
            Err(ShortcutError::NoKey),
            "modifiers only"
        );
        assert_eq!(
            "Shift+H".parse::<Shortcut>(),
            Err(ShortcutError::NoModifier),
            "shift only"
        );
        assert_eq!(
            "H".parse::<Shortcut>(),
            Err(ShortcutError::NoModifier),
            "a bare letter"
        );
        assert_eq!(
            "Win+H+J".parse::<Shortcut>(),
            Err(ShortcutError::TwoKeys),
            "two keys"
        );
        assert!(
            matches!(
                "Win+Alt+Ö".parse::<Shortcut>(),
                Err(ShortcutError::UnknownKey(_))
            ),
            "unknown key"
        );
        assert!(
            matches!(
                "Alt+Win+B".parse::<Shortcut>(),
                Err(ShortcutError::Reserved(_))
            ),
            "Windows' own HDR shortcut"
        );
        assert!(
            "Ctrl+Win+Alt+B".parse::<Shortcut>().is_ok(),
            "only the exact one is reserved"
        );
    }

    #[test]
    fn bindings_round_trip_through_the_file() {
        #[derive(Serialize, Deserialize)]
        struct File {
            shortcuts: Shortcuts,
        }
        let text =
            "[shortcuts]\ntoggle_hdr = \"ctrl+alt+h\"\ncycle_mode = \"\"\npause = \"nonsense\"\n";
        let file: File = toml::from_str(text).expect("parses");
        let s = &file.shortcuts;
        assert_eq!(s.toggle_hdr.text(), "Ctrl+Alt+H", "tidied");
        assert_eq!(s.cycle_mode, Binding::Off, "empty is off");
        assert_eq!(
            s.pause,
            Binding::Invalid("nonsense".into()),
            "kept as written"
        );
        let [hdr, mode, pause] = s.resolved();
        assert_eq!(
            hdr.map(|k| k.to_string()).as_deref(),
            Some("Ctrl+Alt+H"),
            "own keys"
        );
        assert_eq!(mode, None, "off");
        assert_eq!(pause, Some(DEFAULT_PAUSE), "invalid uses the default");
        let back = toml::to_string(&file).expect("writes");
        assert!(
            back.contains("pause = \"nonsense\""),
            "invalid text is not lost: {back}"
        );
        let empty: File = toml::from_str("[shortcuts]\n").expect("parses");
        assert!(
            empty.shortcuts.is_default(),
            "missing keys are the defaults"
        );
    }
}
