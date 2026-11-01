//! Problem reports on Windows: recording a crash, offering on the next start
//! to copy the details, and copying them. Huedra sends nothing itself; the
//! person pastes the details wherever they report the problem.
//! Feature doc: `docs/features/14-problem-reports.md`.

use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use huedra_core::locations::Locations;
use huedra_core::report::{self, Details};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Diagnostics::Debug::{EXCEPTION_POINTERS, SetUnhandledExceptionFilter};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_MESSAGE, IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::registry::{self, Hive};
use crate::{clock, shell};

/// `CF_UNICODETEXT`: UTF-16 text on the clipboard.
const CLIPBOARD_TEXT: u32 = 13;
/// What an unhandled-exception filter returns to let Windows end the
/// process as it would without one.
const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
/// Addresses this far past the program's start are shown as an offset into
/// it, which stays the same between runs.
const MAX_IMAGE_OFFSET: usize = 64 << 20;
const VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

const OFFER_TITLE: &str = "Huedra closed unexpectedly";
const OFFER_TEXT: &str = "Huedra stopped after an error the last time it ran.\n\n\
    Copy the details to the clipboard and open the log folder, so you can \
    send them with a problem report? Huedra itself sends nothing.";

/// Where a Windows-level crash is written, and which part of Huedra it was.
static CRASH_FILE: OnceLock<(PathBuf, &'static str)> = OnceLock::new();

/// Writes a crash file when the process ends on a Windows exception (an
/// access violation, for example). Rust panics are written by the panic
/// hook instead. Best effort: a damaged process may not manage to write.
pub fn record_native_crashes(path: PathBuf, role: &'static str) {
    if CRASH_FILE.set((path, role)).is_ok() {
        // SAFETY: installs a process-wide filter; the function is static and
        // stays valid for the life of the process.
        let _ = unsafe { SetUnhandledExceptionFilter(Some(on_unhandled_exception)) };
    }
}

unsafe extern "system" fn on_unhandled_exception(info: *const EXCEPTION_POINTERS) -> i32 {
    if let Some((path, role)) = CRASH_FILE.get() {
        // SAFETY: Windows passes the filter a pointer to valid exception
        // information (or null, which `as_ref` turns into `None`).
        let record = unsafe { info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) };
        let message = record.map_or_else(
            || "a Windows exception".to_owned(),
            |r| {
                format!(
                    "Windows exception 0x{:08X} at {}",
                    r.ExceptionCode.0,
                    address(r.ExceptionAddress)
                )
            },
        );
        let entry = report::crash_entry(&clock::now().to_string(), role, &message);
        let _ = fs::write(path, entry);
    }
    EXCEPTION_CONTINUE_SEARCH
}

/// "huedra.exe+0x1234" when the address is inside the program.
fn address(at: *mut c_void) -> String {
    let at = at.addr();
    // SAFETY: the handle of this program's own module; nothing to free.
    let base = unsafe { GetModuleHandleW(None) }.map_or(0, |m| m.0.addr());
    match at.checked_sub(base) {
        Some(offset) if base != 0 && offset < MAX_IMAGE_OFFSET => {
            format!("huedra.exe+0x{offset:X}")
        }
        _ => format!("0x{at:X}"),
    }
}

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

/// Copies the problem report to the clipboard and shows the log in
/// Explorer. Returns whether the copy worked.
pub fn copy_details(locations: &Locations) -> bool {
    let copied = copy_text(&details(locations));
    shell::show_in_folder(&locations.log);
    copied
}

/// The report: versions, the newest crash and the newest log lines, with
/// the profile folder hidden.
fn details(locations: &Locations) -> String {
    let windows = report::windows_name(
        registry::string(Hive::LocalMachine, VERSION_KEY, "CurrentBuild").as_deref(),
        registry::string(Hive::LocalMachine, VERSION_KEY, "DisplayVersion").as_deref(),
    );
    let crash = fs::read_to_string(&locations.crash)
        .or_else(|_| fs::read_to_string(&locations.last_crash))
        .ok();
    let mut rotated = locations.log.clone().into_os_string();
    rotated.push(".1");
    let read = |path: PathBuf| {
        fs::read(path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    };
    let log = read(PathBuf::from(rotated)) + &read(locations.log.clone());
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
