//! Window titles, full-screen state and displays of video programs and
//! games, and per-process WinEvent hooks that report when they change. The
//! hooks run out of context and are limited to one process each, so other
//! programs' changes never wake Huedra. Feature doc:
//! `docs/features/05-video.md`.

use std::mem::size_of;

use huedra_core::video::VideoWindow;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EnumWindows, GA_ROOT, GWL_STYLE,
    GetAncestor, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, WINEVENT_OUTOFCONTEXT, WS_CAPTION,
};
use windows::core::BOOL;

use crate::display::{gdi_number, wide_to_string};
use crate::event_loop::{Event, push};

/// `OBJID_WINDOW`: the event concerns the window itself.
const OBJID_WINDOW: i32 = 0;
/// `CHILDID_SELF`: not one of its parts.
const CHILDID_SELF: i32 = 0;
/// Longest title read; video titles are far shorter.
const MAX_TITLE: i32 = 1024;

/// Which changes a [`ProcessHook`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    /// Window titles.
    Titles,
    /// Window moves and size changes, such as entering full screen.
    Moves,
}

/// A WinEvent hook for the top-level windows of one process; removed on
/// drop. Changes arrive as [`Event::WindowChanged`] with the process id.
#[derive(Debug)]
pub struct ProcessHook(HWINEVENTHOOK);

impl ProcessHook {
    /// Installs a hook for `pid`. Call on the event loop's thread.
    pub fn install(pid: u32, watch: Watch) -> Option<Self> {
        let event = match watch {
            Watch::Titles => EVENT_OBJECT_NAMECHANGE,
            Watch::Moves => EVENT_OBJECT_LOCATIONCHANGE,
        };
        // SAFETY: an out-of-context hook limited to one process; Windows
        // calls `on_change` on this thread while it waits for messages.
        let hook = unsafe {
            SetWinEventHook(
                event,
                event,
                None,
                Some(on_change),
                pid,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        (!hook.0.is_null()).then_some(Self(hook))
    }
}

impl Drop for ProcessHook {
    fn drop(&mut self) {
        // SAFETY: the hook was installed by this thread and is removed once.
        let _ = unsafe { UnhookWinEvent(self.0) };
    }
}

unsafe extern "system" fn on_change(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // Only the windows themselves; browsers report many changes to their
    // inner parts.
    if id_object != OBJID_WINDOW || id_child != CHILDID_SELF || hwnd.0.is_null() {
        return;
    }
    // SAFETY: a stale window only gives a null result.
    if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
        return;
    }
    let mut pid = 0_u32;
    // SAFETY: `pid` is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid != 0 {
        push(Event::WindowChanged(pid));
    }
}

struct Search {
    pid: u32,
    found: Vec<VideoWindow>,
}

/// The visible, titled top-level windows of a process.
pub fn windows_of(pid: u32) -> Vec<VideoWindow> {
    let mut search = Search {
        pid,
        found: Vec::new(),
    };
    // SAFETY: `search` outlives the call; the callback only uses it during
    // the enumeration.
    let _ = unsafe {
        EnumWindows(
            Some(collect),
            LPARAM(std::ptr::from_mut(&mut search) as isize),
        )
    };
    search.found
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `Search` passed by `windows_of`, alive and
    // not otherwise borrowed during the enumeration.
    let search = unsafe { &mut *(lparam.0 as *mut Search) };
    let mut pid = 0_u32;
    // SAFETY: `pid` is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    // SAFETY: plain queries on a window handle from the enumeration.
    let shown = unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
    if pid == search.pid
        && shown
        && let Some(title) = title(hwnd)
    {
        let monitor = monitor_of(hwnd);
        search.found.push(VideoWindow {
            title,
            full_screen: monitor.is_some_and(|(area, _)| is_full_screen(hwnd, area)),
            monitor: monitor.map(|(_, number)| number).filter(|&n| n != 0),
        });
    }
    BOOL::from(true)
}

struct Largest {
    pid: u32,
    /// Area and display number of the largest window so far.
    best: Option<(i64, u32)>,
}

/// The Windows number of the display showing a process's largest visible
/// top-level window, such as a game's main window; `None` while it has
/// none.
pub fn main_monitor(pid: u32) -> Option<u32> {
    let mut search = Largest { pid, best: None };
    // SAFETY: `search` outlives the call; the callback only uses it during
    // the enumeration.
    let _ = unsafe {
        EnumWindows(
            Some(largest),
            LPARAM(std::ptr::from_mut(&mut search) as isize),
        )
    };
    search.best.map(|(_, number)| number).filter(|&n| n != 0)
}

unsafe extern "system" fn largest(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `Largest` passed by `main_monitor`, alive and
    // not otherwise borrowed during the enumeration.
    let search = unsafe { &mut *(lparam.0 as *mut Largest) };
    let mut pid = 0_u32;
    // SAFETY: `pid` is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    // SAFETY: plain queries on a window handle from the enumeration.
    let shown = unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid out-parameter.
    if pid != search.pid || !shown || unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return BOOL::from(true);
    }
    let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
    if area > 0
        && search.best.is_none_or(|(best, _)| area > best)
        && let Some((_, number)) = monitor_of(hwnd)
    {
        search.best = Some((area, number));
    }
    BOOL::from(true)
}

fn title(hwnd: HWND) -> Option<String> {
    // SAFETY: a plain query; 0 for no title or a stale window.
    let len = unsafe { GetWindowTextLengthW(hwnd) }.clamp(0, MAX_TITLE);
    if len == 0 {
        return None;
    }
    let mut buffer = vec![0_u16; usize::try_from(len).unwrap_or(0) + 1];
    // SAFETY: Windows writes at most `buffer.len()` characters, with the
    // terminating zero.
    let got = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    buffer.truncate(usize::try_from(got).unwrap_or(0));
    (!buffer.is_empty()).then(|| String::from_utf16_lossy(&buffer))
}

/// The display a window is on (the one it overlaps most): its area and
/// its Windows number, 0 when the name has no number.
fn monitor_of(hwnd: HWND) -> Option<(RECT, u32)> {
    // SAFETY: always returns a monitor with this flag.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = u32::try_from(size_of::<MONITORINFOEXW>()).unwrap_or(0);
    // SAFETY: `info` is a MONITORINFOEXW with its size set, which the
    // function accepts in place of a MONITORINFO and fills completely.
    let ok =
        unsafe { GetMonitorInfoW(monitor, std::ptr::from_mut(&mut info).cast::<MONITORINFO>()) };
    ok.as_bool().then(|| {
        (
            info.monitorInfo.rcMonitor,
            gdi_number(&wide_to_string(&info.szDevice)),
        )
    })
}

/// A window is full screen when it covers its whole monitor (`area`) and
/// has no title bar. The second test excludes maximized windows, whose
/// frame reaches past the monitor's edges.
fn is_full_screen(hwnd: HWND, area: RECT) -> bool {
    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid out-parameter.
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return false;
    }
    // SAFETY: a plain query on a window handle.
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) }.cast_unsigned();
    if style & WS_CAPTION.0 == WS_CAPTION.0 {
        return false;
    }
    let m = area;
    rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
}
