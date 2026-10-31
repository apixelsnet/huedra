//! Whether a program is playing sound, read from the Windows audio sessions
//! (Core Audio). SDR video counts for RTX Video HDR only while its program
//! plays sound. Feature doc: `docs/features/05-video.md`.

use windows::Win32::Media::Audio::{
    AudioSessionStateActive, DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionManager2,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::Interface as _;

use crate::shell;

/// Whether a process for which `matches` returns true has an active audio
/// session on any output device. Errors count as no sound.
pub fn playing(matches: impl Fn(u32) -> bool) -> bool {
    let _com = shell::init_com();
    // SAFETY: creates the system's audio device list in this process.
    let devices: windows::core::Result<IMMDeviceEnumerator> =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) };
    let Ok(devices) = devices else {
        return false;
    };
    // SAFETY: lists the active output devices; the list owns its items.
    let Ok(outputs) = (unsafe { devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }) else {
        return false;
    };
    // SAFETY: a plain query on the list.
    let count = unsafe { outputs.GetCount() }.unwrap_or(0);
    (0..count).any(|index| {
        // SAFETY: `index` is below the list's count.
        unsafe { outputs.Item(index) }.is_ok_and(|device| device_playing(&device, &matches))
    })
}

fn device_playing(device: &IMMDevice, matches: &impl Fn(u32) -> bool) -> bool {
    // SAFETY: asks the device for its session manager; no activation
    // parameters are needed.
    let manager: windows::core::Result<IAudioSessionManager2> =
        unsafe { device.Activate(CLSCTX_ALL, None) };
    let Ok(manager) = manager else {
        return false;
    };
    // SAFETY: a snapshot of the device's sessions.
    let Ok(sessions) = (unsafe { manager.GetSessionEnumerator() }) else {
        return false;
    };
    // SAFETY: a plain query on the snapshot.
    let count = unsafe { sessions.GetCount() }.unwrap_or(0);
    (0..count).any(|index| {
        // SAFETY: `index` is below the snapshot's count.
        let Ok(session) = (unsafe { sessions.GetSession(index) }) else {
            return false;
        };
        // SAFETY: a plain query on the session.
        if unsafe { session.GetState() } != Ok(AudioSessionStateActive) {
            return false;
        }
        let Ok(session) = session.cast::<IAudioSessionControl2>() else {
            return false;
        };
        // SAFETY: a plain query on the session; 0 for system sounds.
        unsafe { session.GetProcessId() }.is_ok_and(|pid| pid != 0 && matches(pid))
    })
}
