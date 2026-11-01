//! Problem reports on Windows: offering on the next start after a crash
//! to copy the details, and copying them. Huedra sends nothing itself; the
//! person pastes the details wherever they report the problem.
//! Feature doc: `docs/features/14-problem-reports.md`.

use std::fs;

use huedra_core::locations::Locations;
use huedra_core::report::{self, Details};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_MESSAGE, IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::registry::{self, Hive};

/// `CF_UNICODETEXT`: UTF-16 text on the clipboard.
const CLIPBOARD_TEXT: u32 = 13;
const VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

const OFFER_TITLE: &str = "Huedra closed unexpectedly";
const OFFER_TEXT: &str = "Huedra stopped after an error last time.\n\n\
    Copy the details for a problem report to the clipboard? \
    Huedra sends nothing itself.";

/// When the last run crashed, asks once whether to copy the details. The
/// question waits on its own thread, so the caller goes on starting.
pub fn offer_after_crash(locations: &Locations) {
    // Moving the file claims the crash, so the tray and the settings window
    // never both ask about it.
    if fs::rename(&locations.crash, &locations.last_crash).is_err() {
        return;
    }
    let locations = locations.clone();
    std::thread::spawn(move || {
        // SAFETY: a modal message box with no owner; both strings outlive it.
        let answer = unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(OFFER_TEXT),
                &HSTRING::from(OFFER_TITLE),
                MB_YESNO | MB_ICONWARNING,
            )
        };
        if answer == IDYES {
            copy_details(&locations);
        }
    });
}

/// Copies the problem report to the clipboard. Returns whether it worked.
/// It opens no Explorer window: the report already holds the newest log
/// lines, and clipboard plus Explorer together made Defender's
/// machine-learning check flag the exe.
pub fn copy_details(locations: &Locations) -> bool {
    copy_text(&details(locations))
}

/// The report: versions, the newest crash and the newest log lines, with
/// the profile folder hidden.
fn details(locations: &Locations) -> String {
    let windows = report::windows_name(
        registry::string(Hive::LocalMachine, VERSION_KEY, "CurrentBuild").as_deref(),
        registry::string(Hive::LocalMachine, VERSION_KEY, "DisplayVersion").as_deref(),
    );
    // The offer on each start has moved any new crash here already.
    let crash = fs::read_to_string(&locations.last_crash).ok();
    // The current file only; it holds the newest lines unless it was just
    // started again. Huedra writes it as UTF-8.
    let log = fs::read_to_string(&locations.log).unwrap_or_default();
    let profile = std::env::var("USERPROFILE").ok();
    report::text(&Details {
        windows: &windows,
        portable: locations.portable,
        crash: crash.as_deref(),
        log: &log,
        profile: profile.as_deref(),
    })
}

/// Puts `text` on the clipboard.
fn copy_text(text: &str) -> bool {
    let utf16: Vec<u16> = text.encode_utf16().chain([0]).collect();
    // The clipboard needs an owner window for `SetClipboardData` to work.
    let Some(owner) = MessageWindow::new() else {
        return false;
    };
    // SAFETY: opens the clipboard for `owner`, which outlives the guard.
    if unsafe { OpenClipboard(Some(owner.0)) }.is_err() {
        return false;
    }
    let _open = OpenClipboardGuard;
    // SAFETY: the clipboard is open by this thread.
    if unsafe { EmptyClipboard() }.is_err() {
        return false;
    }
    let bytes = utf16.len() * size_of::<u16>();
    // SAFETY: allocates movable global memory, as the clipboard requires.
    let Ok(memory) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) }) else {
        return false;
    };
    // SAFETY: locks the block just allocated; null on failure.
    let target = unsafe { GlobalLock(memory) }.cast::<u16>();
    if target.is_null() {
        // SAFETY: the block is ours and not yet handed to the clipboard.
        let _ = unsafe { GlobalFree(Some(memory)) };
        return false;
    }
    // SAFETY: `target` holds `bytes` bytes, exactly `utf16.len()` units; the
    // block is unlocked right after the copy.
    unsafe {
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), target, utf16.len());
        let _ = GlobalUnlock(memory);
    }
    // SAFETY: hands the block to the clipboard, which owns it on success.
    if unsafe { SetClipboardData(CLIPBOARD_TEXT, Some(HANDLE(memory.0))) }.is_err() {
        // SAFETY: the clipboard refused the block, so it is still ours.
        let _ = unsafe { GlobalFree(Some(memory)) };
        return false;
    }
    true
}

/// A hidden message-only window, destroyed when dropped.
struct MessageWindow(HWND);

impl MessageWindow {
    fn new() -> Option<Self> {
        // SAFETY: a message-only window of the system's STATIC class with no
        // name, menu or creation data.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .ok()?;
        Some(Self(hwnd))
    }
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: a window this thread created, destroyed once.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// Closes the clipboard when dropped.
struct OpenClipboardGuard;

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by this thread.
        let _ = unsafe { CloseClipboard() };
    }
}
