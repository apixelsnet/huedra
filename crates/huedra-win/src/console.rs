//! Console handling for a console-subsystem program that also runs in the
//! background. On Windows 11 24H2 and later the manifest's detached console
//! policy means no console is created; on earlier versions this hides the
//! one Windows opened.

use windows::Win32::System::Console::{FreeConsole, GetConsoleProcessList};

/// Releases the console if this process is its only user (it was started
/// from Explorer, not from a shell).
pub fn release_own_console() {
    let mut processes = [0_u32; 2];
    // SAFETY: the buffer is valid for its length.
    let count = unsafe { GetConsoleProcessList(&mut processes) };
    if count == 1 {
        // SAFETY: detaches this process from its console; nothing else uses it.
        let _ = unsafe { FreeConsole() };
    }
}
