//! Display identity and HDR state, shared by the command line, the engine and
//! the settings window. The Windows side fills these in.

use std::fmt;

/// Stable identity of a monitor: the device interface path Windows reports for
/// it. It survives restarts and changes of display order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayId(String);

impl DisplayId {
    /// Wraps a device interface path.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// The device interface path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether HDR is on for a display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrState {
    /// Standard dynamic range.
    Off,
    /// High dynamic range.
    On,
}

impl fmt::Display for HdrState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Off => "off",
            Self::On => "on",
        })
    }
}

/// Whether a display can use HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrSupport {
    /// The display and its connection support HDR.
    Supported,
    /// The display, cable or graphics driver does not support HDR.
    Unsupported,
    /// HDR is supported but Windows currently limits it, for example on battery.
    LimitedByPolicy,
}

impl HdrSupport {
    /// Whether HDR can be requested at all.
    pub fn is_capable(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// How bright SDR content (the desktop, SDR apps) is while HDR is on, as
/// Windows Settings › Display › HDR sets it: 80 to 480 nits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SdrBrightness(u32);

impl SdrBrightness {
    /// The lowest Windows offers.
    pub const MIN: u32 = 80;
    /// The highest Windows offers.
    pub const MAX: u32 = 480;

    /// A brightness in nits, when Windows offers it.
    pub fn from_nits(nits: u32) -> Option<Self> {
        (Self::MIN..=Self::MAX)
            .contains(&nits)
            .then_some(Self(nits))
    }

    /// From Windows' `SDRWhiteLevel`, where 1000 is 80 nits; brought into
    /// range.
    pub fn from_level(level: u32) -> Self {
        let nits = (u64::from(level) * 80 + 500) / 1000;
        Self(
            u32::try_from(nits)
                .unwrap_or(Self::MAX)
                .clamp(Self::MIN, Self::MAX),
        )
    }

    /// The value in nits.
    pub const fn nits(self) -> u32 {
        self.0
    }

    /// As Windows' `SDRWhiteLevel`.
    pub const fn level(self) -> u32 {
        self.0 * 1000 / 80
    }
}

impl std::fmt::Display for SdrBrightness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} nits", self.0)
    }
}

/// Luminance in nits (candela per square metre).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Nits(f32);

impl Nits {
    /// Accepts finite, positive values only; drivers report 0 when unknown.
    pub fn new(value: f32) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    /// The value in nits.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl fmt::Display for Nits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.0} nits", self.0)
    }
}

/// One active display as Huedra sees it. A plain record without invariants,
/// so its fields are public.
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    /// Stable identity.
    pub id: DisplayId,
    /// The number Windows shows for this display under "Identify".
    pub number: u32,
    /// Monitor name from its EDID; may be empty for built-in panels.
    pub name: String,
    /// Whether this is the main display.
    pub primary: bool,
    /// HDR capability.
    pub support: HdrSupport,
    /// Current HDR state.
    pub state: HdrState,
    /// Peak brightness reported by the display, when known.
    pub peak: Option<Nits>,
    /// How bright SDR content is while HDR is on; read only while HDR is
    /// on, as Windows reports nothing useful otherwise.
    pub sdr_brightness: Option<SdrBrightness>,
}

impl Display {
    /// The name to show: the monitor name, or a numbered fallback.
    pub fn label(&self) -> String {
        if self.name.trim().is_empty() {
            format!("Display {}", self.number)
        } else {
            self.name.clone()
        }
    }
}

/// Which displays a command applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySelector {
    /// The main display.
    Primary,
    /// The display with this "Identify" number.
    Number(u32),
    /// Every display that supports HDR.
    AllCapable,
}

/// No display matched a [`DisplaySelector`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectError {
    /// No display carries the requested number.
    #[error("there is no display {0}; run `huedra hdr status` to list them")]
    NoSuchNumber(u32),
    /// Windows reported no primary display.
    #[error("Windows reported no primary display")]
    NoPrimary,
    /// No display supports HDR.
    #[error("no connected display supports HDR")]
    NoneCapable,
}

impl DisplaySelector {
    /// The displays this selector names.
    ///
    /// # Errors
    /// [`SelectError`] when nothing matches.
    pub fn pick(self, displays: &[Display]) -> Result<Vec<&Display>, SelectError> {
        let picked: Vec<&Display> = match self {
            Self::Primary => displays.iter().filter(|d| d.primary).collect(),
            Self::Number(n) => displays.iter().filter(|d| d.number == n).collect(),
            Self::AllCapable => displays.iter().filter(|d| d.support.is_capable()).collect(),
        };
        if !picked.is_empty() {
            return Ok(picked);
        }
        Err(match self {
            Self::Primary => SelectError::NoPrimary,
            Self::Number(n) => SelectError::NoSuchNumber(n),
            Self::AllCapable => SelectError::NoneCapable,
        })
    }
}

/// Decoders for the advanced-color flag words returned by
/// `DisplayConfigGetDeviceInfo`. The bit layouts come from the Windows SDK
/// header `wingdi.h` (10.0.26100); they live here so they can be tested on any
/// platform.
pub mod color_flags {
    use super::{HdrState, HdrSupport};

    const fn bit(value: u32, index: u32) -> bool {
        value & (1 << index) != 0
    }

    /// `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO_2` (Windows 11 24H2 and later).
    /// The state is the user setting, which is what Windows Settings shows.
    pub fn decode_v2(value: u32) -> (HdrSupport, HdrState) {
        let limited_by_policy = bit(value, 3);
        let hdr_supported = bit(value, 4);
        let hdr_user_enabled = bit(value, 5);
        let support = match (hdr_supported, limited_by_policy) {
            (false, _) => HdrSupport::Unsupported,
            (true, true) => HdrSupport::LimitedByPolicy,
            (true, false) => HdrSupport::Supported,
        };
        (support, state(hdr_user_enabled))
    }

    /// `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO` (earlier versions). Advanced
    /// color with wide color enforced is not HDR.
    pub fn decode_v1(value: u32) -> (HdrSupport, HdrState) {
        let supported = bit(value, 0);
        let enabled = bit(value, 1);
        let wide_color_enforced = bit(value, 2);
        let force_disabled = bit(value, 3);
        let support = match (supported, force_disabled) {
            (false, _) => HdrSupport::Unsupported,
            (true, true) => HdrSupport::LimitedByPolicy,
            (true, false) => HdrSupport::Supported,
        };
        (support, state(enabled && !wide_color_enforced))
    }

    fn state(on: bool) -> HdrState {
        if on { HdrState::On } else { HdrState::Off }
    }
}

#[cfg(test)]
mod tests {
    use super::color_flags::{decode_v1, decode_v2};
    use super::*;

    #[test]
    fn sdr_brightness_converts_windows_levels() {
        assert_eq!(
            SdrBrightness::from_level(1000).nits(),
            80,
            "1000 is 80 nits"
        );
        assert_eq!(
            SdrBrightness::from_level(3000).nits(),
            240,
            "3000 is 240 nits"
        );
        assert_eq!(
            SdrBrightness::from_level(0).nits(),
            80,
            "brought into range"
        );
        assert_eq!(
            SdrBrightness::from_level(99_999).nits(),
            480,
            "brought into range"
        );
        assert_eq!(
            SdrBrightness::from_nits(240).map(SdrBrightness::level),
            Some(3000),
            "back"
        );
        assert_eq!(SdrBrightness::from_nits(79), None, "below Windows' range");
        assert_eq!(SdrBrightness::from_nits(481), None, "above Windows' range");
    }

    fn display(number: u32, primary: bool, support: HdrSupport) -> Display {
        Display {
            id: DisplayId::new(format!("path{number}")),
            number,
            name: String::new(),
            primary,
            support,
            state: HdrState::Off,
            peak: None,
            sdr_brightness: None,
        }
    }

    #[test]
    fn v2_reads_user_setting() {
        // supported(0) | active(1) | hdr supported(4) | hdr enabled(5)
        assert_eq!(
            decode_v2(0b11_0011),
            (HdrSupport::Supported, HdrState::On),
            "HDR on"
        );
        assert_eq!(
            decode_v2(0b01_0001),
            (HdrSupport::Supported, HdrState::Off),
            "HDR off"
        );
        assert_eq!(
            decode_v2(0b1100_0001),
            (HdrSupport::Unsupported, HdrState::Off),
            "wide color only"
        );
        assert_eq!(
            decode_v2(0b01_1001),
            (HdrSupport::LimitedByPolicy, HdrState::Off),
            "battery policy"
        );
    }

    #[test]
    fn v1_excludes_wide_color_enforced() {
        assert_eq!(
            decode_v1(0b0011),
            (HdrSupport::Supported, HdrState::On),
            "HDR on"
        );
        assert_eq!(
            decode_v1(0b0111),
            (HdrSupport::Supported, HdrState::Off),
            "wide color enforced is not HDR"
        );
        assert_eq!(
            decode_v1(0b1001),
            (HdrSupport::LimitedByPolicy, HdrState::Off),
            "force disabled"
        );
        assert_eq!(
            decode_v1(0),
            (HdrSupport::Unsupported, HdrState::Off),
            "nothing"
        );
    }

    #[test]
    fn selector_picks_and_reports() {
        let displays = [
            display(1, true, HdrSupport::Supported),
            display(2, false, HdrSupport::Unsupported),
        ];
        let numbers = |sel: DisplaySelector| -> Vec<u32> {
            sel.pick(&displays)
                .map(|v| v.iter().map(|d| d.number).collect())
                .unwrap_or_default()
        };
        assert_eq!(numbers(DisplaySelector::Primary), [1], "primary");
        assert_eq!(numbers(DisplaySelector::Number(2)), [2], "by number");
        assert_eq!(numbers(DisplaySelector::AllCapable), [1], "capable only");
        assert_eq!(
            DisplaySelector::Number(7).pick(&displays),
            Err(SelectError::NoSuchNumber(7)),
            "missing number"
        );
    }

    #[test]
    fn nits_rejects_unknown() {
        assert!(Nits::new(0.0).is_none(), "zero means unknown");
        assert!(Nits::new(f32::NAN).is_none(), "NaN");
        assert_eq!(
            Nits::new(600.4).map(|n| n.to_string()),
            Some("600 nits".into()),
            "formatting"
        );
    }

    #[test]
    fn label_falls_back_to_number() {
        let mut d = display(3, false, HdrSupport::Supported);
        assert_eq!(d.label(), "Display 3", "empty name");
        d.name = "LG ULTRAGEAR".into();
        assert_eq!(d.label(), "LG ULTRAGEAR", "EDID name");
    }
}
