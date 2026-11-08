//! Color profiles (ICC) of the displays through the Windows color system
//! (`mscms.dll`, Windows 10 build 20348 and later): the installed profiles,
//! and each display's default profile for SDR and for HDR.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use huedra_core::color::{self, Change, ColorError, ColorMode, ColorSystem, Installed};
use huedra_core::display::DisplayId;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HLOCAL, LUID, LocalFree};
use windows::Win32::UI::ColorSystem::{
    COLORPROFILESUBTYPE, CPST_EXTENDED_DISPLAY_COLOR_MODE, CPST_STANDARD_DISPLAY_COLOR_MODE,
    CPT_ICC, ColorProfileAddDisplayAssociation, ColorProfileGetDisplayDefault,
    ColorProfileGetDisplayUserScope, ColorProfileRemoveDisplayAssociation,
    ColorProfileSetDisplayDefaultAssociation, GetColorDirectoryW, WCS_PROFILE_MANAGEMENT_SCOPE,
    WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
};
use windows::core::{HRESULT, HSTRING, PCWSTR, PWSTR};

use crate::display::{self, DisplayError};
use crate::registry::{self, Hive};

/// Most entries of the color folder looked at.
const MAX_ENTRIES: usize = 2000;

/// The display profiles installed in the Windows color folder.
pub fn installed() -> Installed {
    let mut found = Installed::default();
    let Some(entries) = color_directory().and_then(|dir| fs::read_dir(dir).ok()) else {
        return found;
    };
    for entry in entries.flatten().take(MAX_ENTRIES) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !color::is_profile_name(&name) || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let Some(kind) = read_header(&entry.path()).and_then(|h| color::inspect(&h)) else {
            continue;
        };
        if !kind.display {
            continue;
        }
        if kind.hdr {
            found.hdr.push(name.clone());
        }
        found.sdr.push(name);
    }
    found.sorted()
}

fn read_header(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|f| f.take(color::HEADER_BYTES as u64).read_to_end(&mut bytes))
        .ok()?;
    Some(bytes)
}

/// `%WINDIR%\System32\spool\drivers\color`, as Windows reports it.
fn color_directory() -> Option<PathBuf> {
    let mut buffer = [0_u16; 260];
    let mut size = u32::try_from(size_of_val(&buffer)).ok()?;
    // SAFETY: `buffer` holds `size` bytes and outlives the call; Windows
    // writes at most that many.
    let ok = unsafe {
        GetColorDirectoryW(
            PCWSTR::null(),
            Some(PWSTR(buffer.as_mut_ptr())),
            &raw mut size,
        )
    };
    ok.as_bool()
        .then(|| PathBuf::from(display::wide_to_string(&buffer)))
}

/// The displays' default profiles, with the record of changes saved to
/// `record` (`color.toml`).
pub struct Displays {
    record: PathBuf,
    /// The last error writing the record, for the log.
    pub problem: Option<String>,
}

impl Displays {
    /// Works on the displays, saving the record of changes to `record`.
    pub fn new(record: PathBuf) -> Self {
        Self {
            record,
            problem: None,
        }
    }
}

/// Where a display's profiles are kept: its adapter, source, and whether
/// the user has own settings for it ("Use my settings for this device").
fn locate(display: &DisplayId) -> Result<(LUID, u32, WCS_PROFILE_MANAGEMENT_SCOPE), ColorError> {
    let (adapter, source) = display::source_of(display).map_err(|error| match error {
        DisplayError::Gone => ColorError::Gone,
        other => ColorError::Failed(other.to_string()),
    })?;
    // SAFETY: plain query with values from the display configuration.
    let scope = unsafe { ColorProfileGetDisplayUserScope(adapter, source) }
        .unwrap_or(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER);
    Ok((adapter, source, scope))
}

fn subtype(mode: ColorMode) -> COLORPROFILESUBTYPE {
    match mode {
        ColorMode::Sdr => CPST_STANDARD_DISPLAY_COLOR_MODE,
        ColorMode::Hdr => CPST_EXTENDED_DISPLAY_COLOR_MODE,
    }
}

/// The monitors' device class; each monitor's lists of profiles are kept
/// under its numbered key in this class (`...\0001`).
const MONITOR_CLASS: &str = "{4d36e96e-e325-11ce-bfc1-08002be10318}";
/// The lists of the user's own settings for each monitor.
const USER_LISTS: &str =
    r"Software\Microsoft\Windows NT\CurrentVersion\ICM\ProfileAssociations\Display";
/// The lists for every user: the monitor class's driver keys.
const SYSTEM_LISTS: &str = r"SYSTEM\CurrentControlSet\Control\Class";

/// Whether `profile` is on the display's list for `mode`, read from where
/// Windows keeps the lists (not a documented interface; the display color
/// API reads only the SDR list). `None` when the list cannot be read.
fn listed(
    display: &DisplayId,
    scope: WCS_PROFILE_MANAGEMENT_SCOPE,
    mode: ColorMode,
    profile: &str,
) -> Option<bool> {
    // `\\?\DISPLAY#GSM5BD3#5&1&UID4357#{guid}` is the monitor
    // `DISPLAY\GSM5BD3\5&1&UID4357`; its `Driver` value names its key in
    // the monitor class, for example `{4d36e96e-...}\0001`.
    let path = display.as_str().strip_prefix(r"\\?\")?;
    let mut parts = path.split('#');
    let instance = [parts.next()?, parts.next()?, parts.next()?].join(r"\");
    let enum_key = format!(r"SYSTEM\CurrentControlSet\Enum\{instance}");
    let driver = registry::string(Hive::LocalMachine, &enum_key, "Driver")?;
    let number = driver.strip_prefix(MONITOR_CLASS)?.strip_prefix('\\')?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (hive, key) = if scope == WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER {
        (Hive::CurrentUser, USER_LISTS)
    } else {
        (Hive::LocalMachine, SYSTEM_LISTS)
    };
    let value = match mode {
        ColorMode::Sdr => "ICMProfile",
        ColorMode::Hdr => "ICMProfileAC",
    };
    let names = registry::strings(hive, &format!(r"{key}\{MONITOR_CLASS}\{number}"), value)?;
    Some(names.iter().any(|n| n.eq_ignore_ascii_case(profile)))
}

fn failed(error: &windows::core::Error) -> ColorError {
    ColorError::Failed(format!("Windows refused (error {:#010x})", error.code().0))
}

impl ColorSystem for Displays {
    fn default_profile(
        &mut self,
        display: &DisplayId,
        mode: ColorMode,
    ) -> Result<Option<String>, ColorError> {
        let (adapter, source, scope) = locate(display)?;
        // SAFETY: plain query; Windows allocates the name, freed below.
        match unsafe {
            ColorProfileGetDisplayDefault(scope, adapter, source, CPT_ICC, subtype(mode))
        } {
            Ok(name) => {
                // SAFETY: on success the name is a terminated string.
                let text = unsafe { name.to_string() }.ok();
                // SAFETY: Windows allocated it with LocalAlloc; not used after.
                unsafe { LocalFree(Some(HLOCAL(name.0.cast()))) };
                Ok(text)
            }
            // The list is empty: no default.
            Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => Ok(None),
            Err(error) => Err(failed(&error)),
        }
    }

    fn use_profile(
        &mut self,
        display: &DisplayId,
        mode: ColorMode,
        profile: &str,
    ) -> Result<bool, ColorError> {
        let (adapter, source, scope) = locate(display)?;
        // Setting a default also puts the profile on the display's list, so
        // whether it was there is read beforehand. When the list cannot be
        // read the profile counts as listed, and is not taken off later.
        let before = listed(display, scope, mode, profile);
        let name = HSTRING::from(profile);
        // SAFETY: `name` outlives the call; the other values come from the
        // display configuration.
        let set = unsafe {
            ColorProfileSetDisplayDefaultAssociation(
                scope,
                &name,
                CPT_ICC,
                subtype(mode),
                adapter,
                source,
            )
        };
        if set.is_ok() {
            return Ok(before == Some(false));
        }
        // Refused: added to the display's list as its default instead.
        // SAFETY: as above.
        unsafe {
            ColorProfileAddDisplayAssociation(
                scope,
                &name,
                adapter,
                source,
                true,
                mode == ColorMode::Hdr,
            )
        }
        .map_err(|e| failed(&e))?;
        Ok(before != Some(true))
    }

    fn remove_profile(
        &mut self,
        display: &DisplayId,
        mode: ColorMode,
        profile: &str,
    ) -> Result<(), ColorError> {
        let (adapter, source, scope) = locate(display)?;
        let name = HSTRING::from(profile);
        // SAFETY: `name` outlives the call; the other values come from the
        // display configuration.
        unsafe {
            ColorProfileRemoveDisplayAssociation(
                scope,
                &name,
                adapter,
                source,
                mode == ColorMode::Hdr,
            )
        }
        .map_err(|e| failed(&e))
    }

    fn save(&mut self, record: &[Change]) {
        self.problem = color::save_record(&self.record, record)
            .err()
            .map(|e| e.to_string());
    }
}
