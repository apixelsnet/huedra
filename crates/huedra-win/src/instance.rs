//! Keeps the settings window single: a second request brings the open
//! window to the front instead. Also checks remembered positions against the
//! connected monitors.

use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow,
};
use windows::core::{HSTRING, PCWSTR};

/// Brings the top-level window with this exact title to the front. Returns
/// `false` when there is none.
pub fn activate_window(title: &str) -> bool {
    // SAFETY: looks up a top-level window by title; strings outlive the call.
    let Ok(hwnd) = (unsafe { FindWindowW(PCWSTR::null(), &HSTRING::from(title)) }) else {
        return false;
    };
    if hwnd.0.is_null() {
        return false;
    }
    // SAFETY: `hwnd` is a window handle just returned by Windows; a window
    // that closed meanwhile only makes the calls fail.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
    true
}

/// Whether a point (physical pixels) lies on a connected monitor.
pub fn on_screen(x: i32, y: i32) -> bool {
    // SAFETY: a plain query with a point argument.
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONULL) };
    !monitor.0.is_null()
}
