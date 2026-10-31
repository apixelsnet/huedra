//! Notices which program comes to the front, through an out-of-context
//! WinEvent hook: no code is placed in other processes.
//! Feature doc: `docs/features/03-watchers.md`.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetWindowThreadProcessId, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};

use crate::event_loop::{Event, push};

/// The foreground hook; removed on drop. Events arrive as
/// [`Event::Foreground`] on the thread that installed it.
#[derive(Debug)]
pub struct ForegroundHook(HWINEVENTHOOK);

impl ForegroundHook {
    /// Installs the hook. Call on the event loop's thread.
    pub fn install() -> Option<Self> {
        // SAFETY: an out-of-context hook; Windows calls `on_foreground` on
        // this thread while it waits for messages.
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_foreground),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        (!hook.0.is_null()).then_some(Self(hook))
    }
}

impl Drop for ForegroundHook {
    fn drop(&mut self) {
        // SAFETY: the hook was installed by this thread and is removed once.
        let _ = unsafe { UnhookWinEvent(self.0) };
    }
}

/// The process owning the current foreground window.
pub fn foreground_pid() -> Option<u32> {
    // SAFETY: no arguments; may return a null window.
    window_pid(unsafe { GetForegroundWindow() })
}

fn window_pid(hwnd: HWND) -> Option<u32> {
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid = 0_u32;
    // SAFETY: `pid` is a valid out-parameter; a stale window only gives 0.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if let Some(pid) = window_pid(hwnd) {
        push(Event::Foreground(pid));
    }
}
