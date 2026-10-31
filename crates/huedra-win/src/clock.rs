//! Local wall-clock time for the history list and the log.

use huedra_core::clock::LocalTime;
use windows::Win32::System::SystemInformation::GetLocalTime;

/// The current local time.
pub fn now() -> LocalTime {
    // SAFETY: no arguments; returns a plain structure.
    let t = unsafe { GetLocalTime() };
    LocalTime {
        year: t.wYear,
        month: t.wMonth,
        day: t.wDay,
        hour: t.wHour,
        minute: t.wMinute,
        second: t.wSecond,
    }
}
