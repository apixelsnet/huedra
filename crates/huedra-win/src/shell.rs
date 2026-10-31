//! Small shell services for the settings window: opening a web page, the
//! file picker, and the Windows "Animation effects" setting.

use std::mem::size_of;

use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

/// Opens an `https://` address in the default browser. Other schemes are
/// refused, so nothing else can be started this way.
pub fn open_web_page(url: &str) -> bool {
    if !url.starts_with("https://") {
        return false;
    }
    // SAFETY: opens a URL with the default handler; strings outlive the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(url),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    result.0 as usize > 32
}

/// Asks the user for a program (`.exe`). Returns its path.
pub fn pick_program(title: &str) -> Option<String> {
    let mut file = vec![0_u16; 4096];
    let filter: Vec<u16> = "Programs (*.exe)\0*.exe\0"
        .encode_utf16()
        .chain([0])
        .collect();
    let title = HSTRING::from(title);
    let mut dialog = OPENFILENAMEW {
        lStructSize: u32::try_from(size_of::<OPENFILENAMEW>()).unwrap_or(0),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: u32::try_from(file.len()).unwrap_or(0),
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: every buffer referenced by `dialog` outlives the modal call.
    if !unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(String::from_utf16_lossy(&file[..len]))
}

/// Whether Windows "Animation effects" are on. Defaults to on if unknown.
pub fn animations_enabled() -> bool {
    let mut enabled = windows::core::BOOL(1);
    // SAFETY: `enabled` is a BOOL, as this action requires.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut enabled).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_err() || enabled.as_bool()
}
