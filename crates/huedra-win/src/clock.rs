//! Local wall-clock time for the history list and the log, and how long
//! this process has run (start-up time is measured from its creation).

use std::time::Duration;

use huedra_core::clock::LocalTime;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GetLocalTime, GetSystemTimePreciseAsFileTime};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

/// Time since Windows created this process, including the time spent
/// loading it before `main`.
pub fn since_process_start() -> Option<Duration> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: the current-process pseudo-handle needs no closing; the four
    // out-parameters are valid.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    }
    .ok()?;
    // SAFETY: no arguments; returns a plain structure.
    let now = unsafe { GetSystemTimePreciseAsFileTime() };
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    // FILETIME counts 100-nanosecond intervals.
    let elapsed = ticks(now).checked_sub(ticks(created))?;
    Some(Duration::from_nanos(elapsed.saturating_mul(100)))
}

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
