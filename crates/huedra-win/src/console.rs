//! Console handling for a console-subsystem program that also runs in the
//! background. On Windows 11 24H2 and later the manifest's detached console
//! policy means no console is created; on earlier versions, and when the
//! starting process asks for `CREATE_NO_WINDOW`, this hides the one Windows
//! opened.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    FreeConsole, GetConsoleProcessList, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    SetStdHandle,
};

/// Releases the console if this process is its only user (it was started
/// from Explorer or with `CREATE_NO_WINDOW`, not from a shell).
///
/// The standard handles are cleared as well. They pointed into the released
/// console, and a program started later with inherited standard handles
/// (what `std::process::Command` does by default) would fail to start,
/// because those handles can no longer be duplicated. Cleared, children
/// receive none, and this process's own output is discarded.
pub fn release_own_console() {
    let mut processes = [0_u32; 2];
    // SAFETY: the buffer is valid for its length.
    let count = unsafe { GetConsoleProcessList(&mut processes) };
    if count != 1 {
        return;
    }
    // SAFETY: detaches this process from its console; nothing else uses it.
    if unsafe { FreeConsole() }.is_err() {
        return;
    }
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: replaces this process's standard handle with none; the
        // old value referred to the console released above.
        let _ = unsafe { SetStdHandle(which, HANDLE::default()) };
    }
}
