//! Keeps the settings window single: a second request brings the open
//! window to the front instead, and Exit closes every one. Also checks
//! remembered positions against the connected monitors and limits how large
//! the window can be made, and gives it the program's icon.

use std::path::Path;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, EnumThreadWindows, EnumWindows, GetWindowTextW,
    GetWindowThreadProcessId, ICON_BIG, ICON_SMALL, IMAGE_ICON, IsIconic, IsWindowVisible,
    LR_DEFAULTCOLOR, LoadImageW, PostMessageW, SM_CXICON, SM_CXSMICON, SW_RESTORE, SWP_NOSIZE,
    SYSTEM_METRICS_INDEX, SendMessageW, SetForegroundWindow, ShowWindow, WINDOWPOS, WM_CLOSE,
    WM_NCDESTROY, WM_SETICON, WM_WINDOWPOSCHANGING,
};
use windows::core::{BOOL, HSTRING};

use crate::handle::OwnedHandle;

/// Identifies the size limit among a window's subclass procedures.
const LIMIT_SUBCLASS: usize = 0x4855_4544;

/// Brings the settings window (a top-level window of a Huedra process with
/// this exact title) to the front. Returns `false` when there is none.
pub fn activate_window(title: &str) -> bool {
    let Some(&hwnd) = own_windows(title).first() else {
        return false;
    };
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

/// Closes every top-level window of a Huedra process with this exact title,
/// as its close button would. Does nothing when there is none.
pub fn close_window(title: &str) {
    for hwnd in own_windows(title) {
        // SAFETY: posts a message to a window handle just returned by
        // Windows; a window that closed meanwhile only makes the call fail.
        let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }
}

/// Held while this process shows the settings window; dropping it lets
/// another process show one.
#[derive(Debug)]
pub struct WindowClaim {
    _mutex: OwnedHandle,
}

/// Claims the right to show the settings window, so two quick starts (a
/// double click on the tray icon while the PC is still busy after sign-in)
/// cannot both open one before either window exists. `None` when another
/// process holds it.
pub fn claim_window(name: &str) -> Option<WindowClaim> {
    // SAFETY: creates or opens a named mutex; the name outlives the call.
    let handle = unsafe { CreateMutexW(None, false, &HSTRING::from(format!(r"Local\{name}"))) };
    // The thread's last error, as the call just above left it.
    let existed = windows::core::Error::from_thread().code() == ERROR_ALREADY_EXISTS.to_hresult();
    let claim = WindowClaim {
        _mutex: OwnedHandle(handle.ok()?),
    };
    (!existed).then_some(claim)
}

/// Top-level windows with this exact title that belong to a Huedra process:
/// one whose program is named as this one (`huedra.exe`, or
/// `huedra.previous.exe` while an update finishes), in any folder. A File
/// Explorer window open on a folder called "Huedra" has the same title but
/// is not ours.
fn own_windows(title: &str) -> Vec<HWND> {
    let prefix = std::env::current_exe().ok().and_then(|exe| {
        exe.file_stem()
            .map(|stem| format!("{}.", stem.to_string_lossy().to_lowercase()))
    });
    let mut search = Search {
        title: title.encode_utf16().collect(),
        found: Vec::new(),
    };
    // SAFETY: `matching` only reads and pushes into `search`, which
    // outlives the enumeration.
    unsafe {
        let _ = EnumWindows(Some(matching), LPARAM((&raw mut search) as isize));
    }
    let Some(prefix) = prefix else {
        // Nothing to compare with: the title alone decides.
        return search.found;
    };
    search.found.retain(|&hwnd| {
        let mut pid = 0_u32;
        // SAFETY: a plain query on a window handle Windows just gave us.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
        crate::process::image_path(pid).is_some_and(|image| {
            Path::new(&image)
                .file_name()
                .is_some_and(|name| name.to_string_lossy().to_lowercase().starts_with(&prefix))
        })
    });
    search.found
}

struct Search {
    title: Vec<u16>,
    found: Vec<HWND>,
}

unsafe extern "system" fn matching(hwnd: HWND, search: LPARAM) -> BOOL {
    // SAFETY: `search` is the value passed by `own_windows`, alive and not
    // otherwise borrowed while the enumeration runs.
    let search = unsafe { &mut *(search.0 as *mut Search) };
    // One more than the title, so a longer title shows as longer.
    let mut text = vec![0_u16; search.title.len() + 2];
    // SAFETY: `text` holds `text.len()` characters; Windows writes at most
    // that many, including the closing zero.
    let len = unsafe { GetWindowTextW(hwnd, &mut text) };
    if usize::try_from(len).is_ok_and(|len| text[..len] == search.title[..]) {
        search.found.push(hwnd);
    }
    BOOL(1)
}

/// Lets the next process that asks bring its window to the front. Windows
/// gives that right to the program the user just started or clicked in the
/// notification area, and passes it on to a program it starts only when
/// asked. Called before the tray starts the settings window, so a window
/// that is already open comes to the front instead of only flashing on the
/// taskbar. Does nothing when this process holds no such right.
pub fn pass_on_foreground() {
    // SAFETY: a plain call without pointers; it fails harmlessly when this
    // process may not set the foreground window itself.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
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
