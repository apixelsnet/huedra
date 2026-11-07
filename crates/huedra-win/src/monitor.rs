//! The monitor at a point: its work area (the screen minus the taskbar) and
//! its display scale, for placing the tray's quick panel.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

use std::mem::size_of;

use huedra_core::flyout::Rect;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// The work area and scale of a monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    /// The screen minus the taskbar, in physical pixels.
    pub work: Rect,
    /// The whole screen, in physical pixels.
    pub screen: Rect,
    /// The display scale: 1.0 at 100 %, 1.5 at 150 %.
    pub scale: f32,
}

/// The monitor that holds the point, or the nearest one.
pub fn at(x: i32, y: i32) -> Option<Monitor> {
    // SAFETY: a plain query; the default flag always returns a monitor.
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `info` is sized and outlives the call.
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    let (mut dpi_x, mut dpi_y) = (0_u32, 0_u32);
    // SAFETY: both outputs outlive the call.
    let scale =
        match unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } {
            Ok(()) if dpi_x > 0 => f32::from(u16::try_from(dpi_x).unwrap_or(96)) / 96.0,
            _ => 1.0,
        };
    let rect = |r: windows::Win32::Foundation::RECT| Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    };
    Some(Monitor {
        work: rect(info.rcWork),
        screen: rect(info.rcMonitor),
        scale,
    })
}

/// Where the mouse pointer is, in physical pixels.
pub fn pointer() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: `point` outlives the call.
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some((point.x, point.y))
}
