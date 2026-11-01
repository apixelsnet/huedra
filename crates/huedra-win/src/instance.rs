//! Keeps the settings window single: a second request brings the open
//! window to the front instead. Also checks remembered positions against the
//! connected monitors and limits how large the window can be made, and
//! gives it the program's icon.

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, FindWindowW, ICON_BIG, ICON_SMALL, IMAGE_ICON, IsIconic, IsWindowVisible,
    LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SW_RESTORE, SWP_NOSIZE,
    SYSTEM_METRICS_INDEX, SendMessageW, SetForegroundWindow, ShowWindow, WINDOWPOS, WM_NCDESTROY,
    WM_SETICON, WM_WINDOWPOSCHANGING,
};
use windows::core::{BOOL, HSTRING, PCWSTR};

/// Identifies the size limit among a window's subclass procedures.
const LIMIT_SUBCLASS: usize = 0x4855_4544;

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

/// Limits the width and height of the calling thread's visible top-level
/// windows to `max` physical pixels, however they are resized: by the user,
/// by Windows or by another program (which `WM_GETMINMAXINFO` limits do not
/// stop), and gives them the program icon. Returns how many windows were
/// limited; 0 while none is shown yet.
pub fn limit_window_size(max: u16) -> usize {
    let mut windows: Vec<HWND> = Vec::new();
    // SAFETY: `collect` only pushes into `windows`, which outlives the
    // enumeration; the thread id is our own.
    unsafe {
        let _ = EnumThreadWindows(
            GetCurrentThreadId(),
            Some(collect),
            LPARAM((&raw mut windows) as isize),
        );
    }
    for &hwnd in &windows {
        // SAFETY: the windows belong to this thread, as subclassing requires,
        // and the procedure is a plain function that lives forever.
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(limit_size), LIMIT_SUBCLASS, usize::from(max));
        }
        set_icon(hwnd);
    }
    windows.len()
}

/// Resource id of the program icon, written by the exe's build script.
const ICON_RESOURCE: u16 = 1;

/// Gives a window the program's icon, at the sizes Windows draws in the
/// title bar and the task switcher for the window's display scale. The
/// icon resource holds images from 16 to 256 pixels, so each size is
/// drawn sharp rather than scaled from a single image. The icons stay
/// with the window for the rest of the process.
fn set_icon(hwnd: HWND) {
    // SAFETY: a plain query on a window handle of this thread.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    // SAFETY: the module handle of this executable, which needs no closing.
    let Ok(module) = (unsafe { GetModuleHandleW(None) }) else {
        return;
    };
    for (kind, metric) in [(ICON_SMALL, SM_CXSMICON), (ICON_BIG, SM_CXICON)] {
        let size = metric_for(metric, dpi);
        // SAFETY: loads an icon resource of this executable by its integer
        // id (MAKEINTRESOURCE), at an explicit size.
        let icon = unsafe {
            LoadImageW(
                Some(module.into()),
                windows::core::PCWSTR(usize::from(ICON_RESOURCE) as *const u16),
                IMAGE_ICON,
                size,
                size,
                LR_DEFAULTCOLOR,
            )
        };
        if let Ok(HANDLE(icon)) = icon {
            // SAFETY: hands the window an icon handle that stays valid for
            // the life of the process.
            let _ = unsafe {
                SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(kind as usize)),
                    Some(LPARAM(icon as isize)),
                )
            };
        }
    }
}

fn metric_for(metric: SYSTEM_METRICS_INDEX, dpi: u32) -> i32 {
    // SAFETY: a plain query.
    unsafe { GetSystemMetricsForDpi(metric, dpi) }
}

unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
    // SAFETY: `list` is the vector passed by `limit_window_size`, alive and
    // not otherwise borrowed while the enumeration runs.
    let list = unsafe { &mut *(list.0 as *mut Vec<HWND>) };
    // SAFETY: a plain query on a window handle Windows just gave us.
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        list.push(hwnd);
    }
    BOOL(1)
}

unsafe extern "system" fn limit_size(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    max: usize,
) -> LRESULT {
    if message == WM_WINDOWPOSCHANGING {
        // SAFETY: for this message `lparam` points to the WINDOWPOS about to
        // be applied, which the window may change.
        let pos = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
        if !pos.flags.contains(SWP_NOSIZE) {
            let max = i32::try_from(max).unwrap_or(i32::MAX);
            pos.cx = pos.cx.min(max);
            pos.cy = pos.cy.min(max);
        }
    } else if message == WM_NCDESTROY {
        // SAFETY: removes this procedure from the window being destroyed.
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(limit_size), id);
        }
    }
    // SAFETY: hands the message on to the window's own procedure unchanged
    // apart from the size limit above.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
