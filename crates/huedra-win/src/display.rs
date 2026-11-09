//! HDR state of each display through the Windows display configuration (CCD)
//! interface, with peak brightness from DXGI.
//! Feature doc: `docs/features/01-hdr-switch.md`.

use std::collections::HashMap;
use std::mem::size_of;
use std::thread;
use std::time::{Duration, Instant};

use huedra_core::display::{Display, DisplayId, HdrState, HdrSupport, Nits, color_flags};
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE, DISPLAYCONFIG_DEVICE_INFO_TYPE,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS,
    QueryDisplayConfig,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LUID};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput6};
use windows::core::Interface;

// Added in the Windows SDK 10.0.26100 `wingdi.h` (Windows 11 24H2); the
// `windows` crate does not define them yet. Values checked against the header.
const GET_ADVANCED_COLOR_INFO_2: DISPLAYCONFIG_DEVICE_INFO_TYPE =
    DISPLAYCONFIG_DEVICE_INFO_TYPE(15);
const SET_HDR_STATE: DISPLAYCONFIG_DEVICE_INFO_TYPE = DISPLAYCONFIG_DEVICE_INFO_TYPE(16);

const RETRY_DELAY: Duration = Duration::from_millis(300);
/// How long a change may take to show before it counts as not applied.
const APPLY_TIMEOUT: Duration = Duration::from_secs(2);
const APPLY_POLL: Duration = Duration::from_millis(50);
/// The display list can change between sizing and filling the buffers.
const QUERY_ATTEMPTS: u32 = 5;

/// Reading or changing a display failed.
#[derive(Debug, thiserror::Error)]
pub enum DisplayError {
    /// `QueryDisplayConfig` failed.
    #[error("Windows did not return the display configuration (error {0})")]
    Query(u32),
    /// `DisplayConfigGetDeviceInfo` failed for a display.
    #[error("Windows did not describe a display (error {0})")]
    Describe(i32),
    /// The display was disconnected while working on it.
    #[error("the display is no longer connected")]
    Gone,
    /// The display cannot use HDR.
    #[error("{0} does not support HDR")]
    Unsupported(String),
    /// Windows rejected every attempt.
    #[error("Windows refused to change HDR on {name} (error {code})")]
    Refused {
        /// Display label.
        name: String,
        /// Last Win32 error code.
        code: i32,
    },
    /// Windows accepted the request but the state did not change.
    #[error("HDR on {name} did not turn {state} within two seconds")]
    NotApplied {
        /// Display label.
        name: String,
        /// The state that was requested.
        state: HdrState,
    },
}

/// Lists the active displays, ordered by their Windows number.
///
/// # Errors
/// [`DisplayError`] when Windows does not return or describe the displays.
pub fn displays() -> Result<Vec<Display>, DisplayError> {
    let found = enumerate()?;
    let peaks = peak_luminance();
    Ok(found
        .into_iter()
        .map(|f| Display {
            peak: peaks.get(&f.gdi_name).copied(),
            ..f.display
        })
        .collect())
}

/// Lists the active displays without peak brightness, which needs DXGI and
/// is read only when it is shown.
///
/// # Errors
/// [`DisplayError`] when Windows does not return or describe the displays.
pub fn current() -> Result<Vec<Display>, DisplayError> {
    Ok(enumerate()?.into_iter().map(|f| f.display).collect())
}

/// Turns HDR on or off for one display and waits until Windows reports the
/// new state, trying up to `attempts` times (some drivers refuse the first
/// request after a mode change). Does nothing when the display is already in
/// that state.
///
/// # Errors
/// [`DisplayError`] when the display is gone, cannot use HDR, or the change
/// is refused or does not take effect.
pub fn set_hdr(id: &DisplayId, state: HdrState, attempts: u32) -> Result<(), DisplayError> {
    let found = enumerate()?
        .into_iter()
        .find(|f| &f.display.id == id)
        .ok_or(DisplayError::Gone)?;
    let name = found.display.label();
    if !found.display.support.is_capable() {
        return Err(DisplayError::Unsupported(name));
    }
    if found.display.state == state {
        return Ok(());
    }
    let mut last_code = 0;
    for attempt in 0..attempts.max(1) {
        if attempt > 0 {
            thread::sleep(RETRY_DELAY);
        }
        match send(found.target, found.api, state) {
            0 if wait_for(found.target, state) => return Ok(()),
            0 => {}
            code => last_code = code,
        }
    }
    Err(if last_code == 0 {
        DisplayError::NotApplied { name, state }
    } else {
        DisplayError::Refused {
            name,
            code: last_code,
        }
    })
}

/// A display path end: an adapter and a source or target on it.
#[derive(Clone, Copy)]
struct Device {
    adapter: LUID,
    id: u32,
}

/// Which advanced-color interface the system offers.
#[derive(Clone, Copy)]
enum ColorApi {
    /// Windows 11 24H2 and later: separate HDR and wide-color switches.
    V2,
    /// Earlier versions: one advanced-color switch.
    V1,
}

struct Found {
    display: Display,
    target: Device,
    source: Device,
    api: ColorApi,
    gdi_name: String,
}

/// The adapter and source id of a display, as the color profile functions
/// take them.
pub(crate) fn source_of(id: &DisplayId) -> Result<(LUID, u32), DisplayError> {
    enumerate()?
        .into_iter()
        .find(|f| &f.display.id == id)
        .map(|f| (f.source.adapter, f.source.id))
        .ok_or(DisplayError::Gone)
}

fn enumerate() -> Result<Vec<Found>, DisplayError> {
    let (paths, modes) = query_paths()?;
    let mut found = Vec::with_capacity(paths.len());
    for path in &paths {
        let target = Device {
            adapter: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        };
        let source = Device {
            adapter: path.sourceInfo.adapterId,
            id: path.sourceInfo.id,
        };
        let names: DISPLAYCONFIG_TARGET_DEVICE_NAME =
            get_info(target, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME)
                .map_err(DisplayError::Describe)?;
        let gdi: DISPLAYCONFIG_SOURCE_DEVICE_NAME =
            get_info(source, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME)
                .map_err(DisplayError::Describe)?;
        let gdi_name = wide_to_string(&gdi.viewGdiDeviceName);
        let device_path = wide_to_string(&names.monitorDevicePath);
        let (support, state, api) = read_color(target)?;
        found.push(Found {
            display: Display {
                id: DisplayId::new(if device_path.is_empty() {
                    gdi_name.clone()
                } else {
                    device_path
                }),
                number: gdi_number(&gdi_name),
                name: wide_to_string(&names.monitorFriendlyDeviceName),
                primary: is_primary(path, &modes),
                support,
                state,
                peak: None,
            },
            target,
            source,
            api,
            gdi_name,
        });
    }
    found.sort_by_key(|f| f.display.number);
    Ok(found)
}

type PathsAndModes = (Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>);

fn query_paths() -> Result<PathsAndModes, DisplayError> {
    let mut status = ERROR_SUCCESS;
    for _ in 0..QUERY_ATTEMPTS {
        let (mut path_count, mut mode_count) = (0_u32, 0_u32);
        // SAFETY: both pointers refer to live locals.
        status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if status != ERROR_SUCCESS {
            break;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        // SAFETY: the vectors hold exactly `path_count` and `mode_count`
        // elements; Windows writes at most that many and lowers the counts.
        status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };
        if status == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if status != ERROR_SUCCESS {
            break;
        }
        paths.truncate(path_count as usize);
        modes.truncate(mode_count as usize);
        return Ok((paths, modes));
    }
    Err(DisplayError::Query(status.0))
}

/// The primary display's desktop starts at (0, 0).
fn is_primary(path: &DISPLAYCONFIG_PATH_INFO, modes: &[DISPLAYCONFIG_MODE_INFO]) -> bool {
    // SAFETY: without QDC_VIRTUAL_MODE_AWARE the union holds `modeInfoIdx`,
    // and every bit pattern is a valid u32.
    let index = unsafe { path.sourceInfo.Anonymous.modeInfoIdx };
    let Some(mode) = modes.get(index as usize) else {
        return false;
    };
    if mode.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
        return false;
    }
    // SAFETY: `infoType` says the union holds a source mode.
    let position = unsafe { mode.Anonymous.sourceMode.position };
    position.x == 0 && position.y == 0
}

/// `\\.\DISPLAY2` gives 2, the number Windows shows under "Identify".
pub(crate) fn gdi_number(gdi_name: &str) -> u32 {
    gdi_name
        .rsplit("DISPLAY")
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

fn read_color(target: Device) -> Result<(HdrSupport, HdrState, ColorApi), DisplayError> {
    if let Ok(info) = get_info::<ColorInfo2>(target, GET_ADVANCED_COLOR_INFO_2) {
        let (support, state) = color_flags::decode_v2(info.value);
        return Ok((support, state, ColorApi::V2));
    }
    let info = get_info::<ColorInfo1>(target, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO)
        .map_err(DisplayError::Describe)?;
    let (support, state) = color_flags::decode_v1(info.value);
    Ok((support, state, ColorApi::V1))
}

fn send(target: Device, api: ColorApi, state: HdrState) -> i32 {
    let kind = match api {
        ColorApi::V2 => SET_HDR_STATE,
        ColorApi::V1 => DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
    };
    let mut request: SetState = packet(target, kind);
    request.value = u32::from(state == HdrState::On);
    // SAFETY: `SetState` is a `#[repr(C)]` packet that starts with its header,
    // `header.size` is its full size, and Windows only reads it.
    unsafe { DisplayConfigSetDeviceInfo((&raw const request).cast()) }
}

fn wait_for(target: Device, state: HdrState) -> bool {
    let start = Instant::now();
    loop {
        if read_color(target).is_ok_and(|(_, now, _)| now == state) {
            return true;
        }
        if start.elapsed() >= APPLY_TIMEOUT {
            return false;
        }
        thread::sleep(APPLY_POLL);
    }
}

/// Peak luminance per GDI device name (`\\.\DISPLAY1`). Missing entries mean
/// the driver did not report a value; that is not an error.
fn peak_luminance() -> HashMap<String, Nits> {
    let mut peaks = HashMap::new();
    // SAFETY: creates a DXGI factory; the interface releases itself on drop.
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return peaks;
    };
    for adapter_index in 0.. {
        // SAFETY: enumeration by index; an error marks the end of the list.
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(adapter_index) }) else {
            break;
        };
        for output_index in 0.. {
            // SAFETY: as above.
            let Ok(output) = (unsafe { adapter.EnumOutputs(output_index) }) else {
                break;
            };
            let Ok(output) = output.cast::<IDXGIOutput6>() else {
                continue;
            };
            // SAFETY: fills a plain description structure.
            let Ok(desc) = (unsafe { output.GetDesc1() }) else {
                continue;
            };
            if let Some(peak) = Nits::new(desc.MaxLuminance) {
                peaks.insert(wide_to_string(&desc.DeviceName), peak);
            }
        }
    }
    peaks
}

pub(crate) fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}

/// A request for `DisplayConfigGetDeviceInfo` or `DisplayConfigSetDeviceInfo`.
///
/// # Safety
/// Implementors are `#[repr(C)]`, start with a
/// `DISPLAYCONFIG_DEVICE_INFO_HEADER`, and are valid when all zero.
unsafe trait Packet: Default {
    fn header(&mut self) -> &mut DISPLAYCONFIG_DEVICE_INFO_HEADER;
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "packets are a few hundred bytes"
)]
const fn packet_size<P>() -> u32 {
    size_of::<P>() as u32
}

fn packet<P: Packet>(device: Device, kind: DISPLAYCONFIG_DEVICE_INFO_TYPE) -> P {
    let mut packet = P::default();
    let header = packet.header();
    header.r#type = kind;
    header.size = packet_size::<P>();
    header.adapterId = device.adapter;
    header.id = device.id;
    packet
}

fn get_info<P: Packet>(device: Device, kind: DISPLAYCONFIG_DEVICE_INFO_TYPE) -> Result<P, i32> {
    let mut request: P = packet(device, kind);
    // SAFETY: `P` is a `#[repr(C)]` packet that starts with its header (trait
    // contract) and `header.size` is its full size, so Windows writes within it.
    let status = unsafe { DisplayConfigGetDeviceInfo((&raw mut request).cast()) };
    if status == 0 {
        Ok(request)
    } else {
        Err(status)
    }
}

/// `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO`.
#[repr(C)]
#[derive(Default)]
struct ColorInfo1 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
    _color_encoding: i32,
    _bits_per_color_channel: u32,
}

/// `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO_2`.
#[repr(C)]
#[derive(Default)]
struct ColorInfo2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
    _color_encoding: i32,
    _bits_per_color_channel: u32,
    _active_color_mode: i32,
}

/// `DISPLAYCONFIG_SET_HDR_STATE` and `DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE`,
/// which share one layout.
#[repr(C)]
#[derive(Default)]
struct SetState {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
}

// SAFETY: for all five types: `#[repr(C)]`, the header is the first field, and
// all-zero is a valid value (the `windows` types are zeroed by `Default`).
unsafe impl Packet for ColorInfo1 {
    fn header(&mut self) -> &mut DISPLAYCONFIG_DEVICE_INFO_HEADER {
        &mut self.header
    }
}
// SAFETY: see above.
unsafe impl Packet for ColorInfo2 {
    fn header(&mut self) -> &mut DISPLAYCONFIG_DEVICE_INFO_HEADER {
        &mut self.header
    }
}
// SAFETY: see above.
unsafe impl Packet for SetState {
    fn header(&mut self) -> &mut DISPLAYCONFIG_DEVICE_INFO_HEADER {
        &mut self.header
    }
}
// SAFETY: see above.
unsafe impl Packet for DISPLAYCONFIG_TARGET_DEVICE_NAME {
    fn header(&mut self) -> &mut DISPLAYCONFIG_DEVICE_INFO_HEADER {
        &mut self.header
    }
}
// SAFETY: see above.
unsafe impl Packet for DISPLAYCONFIG_SOURCE_DEVICE_NAME {
    fn header(&mut self) -> &mut DISPLAYCONFIG_DEVICE_INFO_HEADER {
        &mut self.header
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Turns HDR off again however the test ends.
    struct RestoreOff(Device);

    impl Drop for RestoreOff {
        fn drop(&mut self) {
            if send(self.0, ColorApi::V2, HdrState::Off) != 0 {
                send(self.0, ColorApi::V1, HdrState::Off);
            }
        }
    }

    fn state_v1(target: Device) -> Option<HdrState> {
        get_info::<ColorInfo1>(target, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO)
            .ok()
            .map(|info| color_flags::decode_v1(info.value).1)
    }

    fn state_v2(target: Device) -> Option<HdrState> {
        get_info::<ColorInfo2>(target, GET_ADVANCED_COLOR_INFO_2)
            .ok()
            .map(|info| color_flags::decode_v2(info.value).1)
    }

    /// Drives the interface used before Windows 11 24H2 on a real HDR display
    /// that is in SDR: on, then off again. It changes the screen, so it runs
    /// only when asked for, with `--ignored`.
    #[test]
    #[ignore = "switches HDR on a real display"]
    fn v1_interface_switches_hdr() -> Result<(), DisplayError> {
        let found = enumerate()?;
        let Some(display) = found
            .iter()
            .find(|f| f.display.support.is_capable() && f.display.state == HdrState::Off)
        else {
            return Ok(());
        };
        let target = display.target;
        let _restore = RestoreOff(target);
        assert_eq!(state_v1(target), Some(HdrState::Off), "v1 reads SDR first");
        for wanted in [HdrState::On, HdrState::Off] {
            let started = Instant::now();
            assert_eq!(send(target, ColorApi::V1, wanted), 0, "v1 request accepted");
            while state_v1(target) != Some(wanted) && started.elapsed() < APPLY_TIMEOUT {
                thread::sleep(APPLY_POLL);
            }
            assert_eq!(state_v1(target), Some(wanted), "v1 reads HDR {wanted}");
            if let Some(v2) = state_v2(target) {
                assert_eq!(v2, wanted, "v2 agrees with v1");
            }
            println!(
                "HDR {wanted} through v1 in {} ms",
                started.elapsed().as_millis()
            );
        }
        Ok(())
    }
}
