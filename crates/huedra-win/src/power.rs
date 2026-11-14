//! Whether the PC runs on battery power. Changes arrive as
//! `WM_POWERBROADCAST` (see [`crate::event_loop::Event::PowerChanged`]).
//! Feature doc: `docs/features/02-engine.md`.

use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

/// Whether the PC runs on battery power now. A PC without a battery, or
/// one Windows cannot tell about, counts as on mains power.
pub fn on_battery() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid out-parameter for the call.
    let read = unsafe { GetSystemPowerStatus(&mut status) }.is_ok();
    // 0 is offline (battery), 1 online, 255 unknown.
    read && status.ACLineStatus == 0
}
