//! Display modes (resolution and refresh rate) through
//! `EnumDisplaySettingsW` and `ChangeDisplaySettingsExW`. Changes are
//! dynamic: Windows does not store them, so [`reset`] (or a restart) brings
//! back the mode set in Windows Settings. Feature doc:
//! `docs/features/02-engine.md`.

use std::mem::size_of;

use huedra_core::display::DisplayId;
use huedra_core::mode::{self, DisplayMode};
use windows::Win32::Graphics::Gdi::{
    CDS_TEST, CDS_TYPE, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_SUCCESSFUL,
    DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS,
    ENUM_DISPLAY_SETTINGS_MODE, EnumDisplaySettingsW,
};
use windows::core::HSTRING;

use crate::display;

/// The modes a display offers in its current state (HDR on or off can
/// change them), each once, largest first.
pub fn offered(id: &DisplayId) -> Vec<DisplayMode> {
    let Some(name) = display::gdi_name(id) else {
        return Vec::new();
    };
    let name = HSTRING::from(name);
    let mut modes = Vec::new();
    for index in 0.. {
        let mut dev = blank();
        // SAFETY: `dev` is a DEVMODEW with its size set; Windows fills it.
        let more =
            unsafe { EnumDisplaySettingsW(&name, ENUM_DISPLAY_SETTINGS_MODE(index), &mut dev) };
        if !more.as_bool() {
            break;
        }
        if dev.dmBitsPerPel == 32
            && let Some(m) =
                DisplayMode::new(dev.dmPelsWidth, dev.dmPelsHeight, dev.dmDisplayFrequency)
        {
            modes.push(m);
        }
    }
    mode::list(modes)
}

/// The mode a display shows now.
pub fn current(id: &DisplayId) -> Option<DisplayMode> {
    let name = HSTRING::from(display::gdi_name(id)?);
    let mut dev = blank();
    // SAFETY: as in `offered`.
    let ok = unsafe { EnumDisplaySettingsW(&name, ENUM_CURRENT_SETTINGS, &mut dev) };
    ok.as_bool()
        .then(|| DisplayMode::new(dev.dmPelsWidth, dev.dmPelsHeight, dev.dmDisplayFrequency))
        .flatten()
}

/// Why a mode was not set.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ModeError {
    /// The display is not connected.
    #[error("the display is no longer connected")]
    Gone,
    /// Windows said the display cannot show it.
    #[error("Windows can't show {0} on this display now (code {1})")]
    Refused(String, i32),
}

/// Sets a mode until [`reset`], checking first that Windows can show it.
///
/// # Errors
/// [`ModeError`] when the display is gone or the mode is refused.
pub fn set(id: &DisplayId, wanted: DisplayMode) -> Result<(), ModeError> {
    let name = HSTRING::from(display::gdi_name(id).ok_or(ModeError::Gone)?);
    let mut dev = blank();
    dev.dmPelsWidth = wanted.width();
    dev.dmPelsHeight = wanted.height();
    dev.dmDisplayFrequency = wanted.hz();
    dev.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    for flags in [CDS_TEST, CDS_TYPE(0)] {
        // SAFETY: `dev` is a valid DEVMODEW that Windows only reads.
        let result = unsafe { ChangeDisplaySettingsExW(&name, Some(&dev), None, flags, None) };
        if result != DISP_CHANGE_SUCCESSFUL {
            return Err(ModeError::Refused(wanted.label(), result.0));
        }
    }
    Ok(())
}

/// Brings back the mode set in Windows Settings.
///
/// # Errors
/// [`ModeError`] when the display is gone or Windows refuses.
pub fn reset(id: &DisplayId) -> Result<(), ModeError> {
    let name = HSTRING::from(display::gdi_name(id).ok_or(ModeError::Gone)?);
    // SAFETY: no mode: Windows returns to the one in its settings.
    let result = unsafe { ChangeDisplaySettingsExW(&name, None, None, CDS_TYPE(0), None) };
    if result == DISP_CHANGE_SUCCESSFUL {
        Ok(())
    } else {
        Err(ModeError::Refused(
            "the mode set in Windows".to_owned(),
            result.0,
        ))
    }
}

fn blank() -> DEVMODEW {
    DEVMODEW {
        dmSize: u16::try_from(size_of::<DEVMODEW>()).unwrap_or(u16::MAX),
        ..Default::default()
    }
}
