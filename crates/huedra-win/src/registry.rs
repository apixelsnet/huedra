//! Registry access: reading store keys, the start-up entry, the "Start with
//! HDR" entry in Explorer's menu, the Windows light or dark app setting and
//! NVIDIA's RTX Video HDR switch.
//! Everything Huedra writes is under `HKEY_CURRENT_USER` and removed by
//! [`remove_all`].
//! Feature docs: `04-game-library.md`, `05-video.md`, `06-launch-with-hdr.md`,
//! `08-settings-window.md`, `12-packaging.md`.

use huedra_core::library::registry::RegistryEntry;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, KEY_WOW64_32KEY,
    REG_NOTIFY_CHANGE_LAST_SET, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    RegCloseKey, RegDeleteKeyValueW, RegDeleteKeyW, RegDeleteTreeW, RegEnumKeyExW, RegGetValueW,
    RegNotifyChangeKeyValue, RegOpenKeyExW, RegQueryInfoKeyW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PWSTR};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Huedra";
/// The "Start with HDR" verb on programs (`exefile`), for this user only.
const MENU_KEY: &str = r"Software\Classes\exefile\shell\Huedra.StartWithHdr";
/// Parents of [`MENU_KEY`], removed when nothing else is in them.
const MENU_PARENTS: [&str; 2] = [
    r"Software\Classes\exefile\shell",
    r"Software\Classes\exefile",
];
const MENU_COMMAND_KEY: &str = r"Software\Classes\exefile\shell\Huedra.StartWithHdr\command";
const THEME_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const THEME_VALUE: &str = "AppsUseLightTheme";
/// The display adapters' device class; each adapter has a numbered subkey
/// (`0000`, `0001`, ...) where its driver keeps settings.
const DISPLAY_CLASS_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
/// NVIDIA's RTX Video HDR switch in an adapter's key: 0 or missing for off,
/// 5 for on. Written by the NVIDIA app and Control Panel; not a documented
/// interface.
const RTX_VIDEO_HDR_VALUE: &str = "_User_Global_VAL_TrueHDR";
const MAX_SUBKEYS: u32 = 4096;
const MAX_STRING_BYTES: u32 = 64 * 1024;

/// Which part of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    /// `HKEY_CURRENT_USER`.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE`, 32-bit view (where the game stores write).
    LocalMachine32,
    /// `HKEY_LOCAL_MACHINE`, native view.
    LocalMachine,
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: opened by `open` and closed once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn open(hive: Hive, path: &str) -> Option<Key> {
    let (root, access) = match hive {
        Hive::CurrentUser => (HKEY_CURRENT_USER, KEY_READ),
        Hive::LocalMachine32 => (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        Hive::LocalMachine => (HKEY_LOCAL_MACHINE, KEY_READ),
    };
    let mut key = HKEY::default();
    // SAFETY: opens a key for reading; `key` receives the handle.
    let status = unsafe { RegOpenKeyExW(root, &HSTRING::from(path), None, access, &mut key) };
    (status == ERROR_SUCCESS).then_some(Key(key))
}

fn read_string(key: &Key, name: &str) -> Option<String> {
    let name = HSTRING::from(name);
    let mut bytes = 0_u32;
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
    // SAFETY: a size query with no buffer.
    let status = unsafe { RegGetValueW(key.0, None, &name, flags, None, None, Some(&mut bytes)) };
    if status != ERROR_SUCCESS || bytes == 0 || bytes > MAX_STRING_BYTES {
        return None;
    }
    let mut buffer = vec![0_u16; (bytes as usize).div_ceil(2)];
    // SAFETY: `buffer` holds `bytes` bytes; Windows writes at most that many.
    let status = unsafe {
        RegGetValueW(
            key.0,
            None,
            &name,
            flags,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

/// A string value, or `None` when the key or value is missing.
pub fn string(hive: Hive, path: &str, name: &str) -> Option<String> {
    read_string(&open(hive, path)?, name)
}

/// Every subkey of `path` with the requested string values.
pub fn subkeys(hive: Hive, path: &str, names: &[&str]) -> Vec<RegistryEntry> {
    let Some(parent) = open(hive, path) else {
        return Vec::new();
    };
    subkey_names(&parent)
        .into_iter()
        .filter_map(|key_name| {
            let child = open(hive, &format!(r"{path}\{key_name}"))?;
            let values = names
                .iter()
                .filter_map(|n| read_string(&child, n).map(|v| ((*n).to_owned(), v)))
                .collect();
            Some(RegistryEntry {
                key: key_name,
                values,
            })
        })
        .collect()
}

/// Whether NVIDIA RTX Video HDR is turned on for any NVIDIA display
/// adapter; `None` when there is none. Reading needs no elevation.
pub fn rtx_video_hdr() -> Option<bool> {
    let parent = open(Hive::LocalMachine, DISPLAY_CLASS_KEY)?;
    let mut found = None;
    for name in subkey_names(&parent) {
        if name.len() != 4 || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Some(adapter) = open(Hive::LocalMachine, &format!(r"{DISPLAY_CLASS_KEY}\{name}"))
        else {
            continue;
        };
        if !read_string(&adapter, "ProviderName").is_some_and(|p| p.contains("NVIDIA")) {
            continue;
        }
        let on = read_dword(&adapter, RTX_VIDEO_HDR_VALUE).is_some_and(|v| v != 0);
        found = Some(found == Some(true) || on);
    }
    found
}

/// The names of a key's subkeys.
fn subkey_names(parent: &Key) -> Vec<String> {
    let mut found = Vec::new();
    for index in 0..MAX_SUBKEYS {
        let mut name = [0_u16; 256];
        let mut len = 256_u32;
        // SAFETY: `name` holds `len` characters; Windows writes the subkey name.
        let status = unsafe {
            RegEnumKeyExW(
                parent.0,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        found.push(String::from_utf16_lossy(&name[..len as usize]));
    }
    found
}

/// The command in the start-up entry, if there is one.
pub fn startup_command() -> Option<String> {
    string(Hive::CurrentUser, RUN_KEY, RUN_VALUE)
}

/// Writes or removes the start-up entry (`HKCU\...\Run\Huedra`).
///
/// # Errors
/// The Windows error when the key cannot be written.
pub fn set_startup(command: Option<&str>) -> windows::core::Result<()> {
    match command {
        Some(command) => write_string(RUN_KEY, RUN_VALUE, command),
        None => {
            let (key, value) = (HSTRING::from(RUN_KEY), HSTRING::from(RUN_VALUE));
            // SAFETY: deletes one value of the current user's Run key.
            match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &key, &value) } {
                ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
                other => other,
            }
            .ok()
        }
    }
}

/// The command of the "Start with HDR" entry, if there is one.
pub fn context_menu_command() -> Option<String> {
    string(Hive::CurrentUser, MENU_COMMAND_KEY, "")
}

/// Adds the "Start with HDR" entry to the menu of programs in Explorer
/// (under "Show more options" on Windows 11), or removes it.
///
/// # Errors
/// The Windows error when the key cannot be written or removed.
pub fn set_context_menu(entry: Option<ContextMenu<'_>>) -> windows::core::Result<()> {
    let Some(entry) = entry else {
        delete_tree(MENU_KEY)?;
        // Writing the entry created these when they were missing.
        for parent in MENU_PARENTS {
            delete_if_empty(parent);
        }
        return Ok(());
    };
    write_string(MENU_KEY, "", entry.text)?;
    write_string(MENU_KEY, "Icon", entry.icon)?;
    write_string(MENU_COMMAND_KEY, "", entry.command)
}

/// The "Start with HDR" entry.
#[derive(Debug, Clone, Copy)]
pub struct ContextMenu<'a> {
    /// The text shown.
    pub text: &'a str,
    /// The program whose icon is shown.
    pub icon: &'a str,
    /// The command run, with `%1` for the program clicked.
    pub command: &'a str,
}

/// Removes every key and value Huedra wrote: the start-up entry and the
/// "Start with HDR" entry. Missing ones are not an error.
///
/// # Errors
/// The first Windows error.
pub fn remove_all() -> windows::core::Result<()> {
    let startup = set_startup(None);
    let menu = set_context_menu(None);
    startup.and(menu)
}

/// Writes a string value under `HKEY_CURRENT_USER`, creating the key.
fn write_string(path: &str, name: &str, text: &str) -> windows::core::Result<()> {
    let data: Vec<u16> = text.encode_utf16().chain([0]).collect();
    let bytes = u32::try_from(data.len() * 2).unwrap_or(u32::MAX);
    // SAFETY: `data` is a null-terminated UTF-16 string of `bytes` bytes.
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(path),
            &HSTRING::from(name),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            bytes,
        )
    }
    .ok()
}

/// Deletes a key under `HKEY_CURRENT_USER` that has no subkeys and no values.
fn delete_if_empty(path: &str) {
    let Some(key) = open(Hive::CurrentUser, path) else {
        return;
    };
    let (mut subkeys, mut values) = (0_u32, 0_u32);
    // SAFETY: asks only for the two counts.
    let status = unsafe {
        RegQueryInfoKeyW(
            key.0,
            None,
            None,
            None,
            Some(&raw mut subkeys),
            None,
            None,
            Some(&raw mut values),
            None,
            None,
            None,
            None,
        )
    };
    drop(key);
    if status == ERROR_SUCCESS && subkeys == 0 && values == 0 {
        // SAFETY: deletes an empty key of the current user's registry.
        let _ = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(path)) };
    }
}

/// Deletes a key under `HKEY_CURRENT_USER` with everything below it.
fn delete_tree(path: &str) -> windows::core::Result<()> {
    // SAFETY: deletes one key of the current user's registry that Huedra
    // created.
    match unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(path)) } {
        ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
        other => other,
    }
    .ok()
}

/// Whether Windows asks apps for their light appearance (Settings >
/// Personalization > Colors). Light when the value is missing, as Windows
/// itself assumes.
pub fn apps_use_light_theme() -> bool {
    let Some(key) = open(Hive::CurrentUser, THEME_KEY) else {
        return true;
    };
    read_dword(&key, THEME_VALUE).is_none_or(|value| value != 0)
}

/// Calls `changed` with [`apps_use_light_theme`] each time the Windows
/// setting changes, on a thread of its own that lives as long as the
/// process. Returns `false` when the setting cannot be watched.
pub fn watch_light_theme(changed: impl Fn(bool) + Send + 'static) -> bool {
    let mut key = HKEY::default();
    // SAFETY: opens a key for change notifications; `key` receives the handle.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(THEME_KEY),
            None,
            KEY_NOTIFY,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return false;
    }
    let key = Watched(Key(key));
    std::thread::Builder::new()
        .name("huedra-theme".into())
        .spawn(move || {
            // Moves the whole key in (a closure would otherwise take only
            // the handle field, which is not `Send`).
            let key = key;
            let mut light = apps_use_light_theme();
            loop {
                // SAFETY: waits on our own open key until a value under it
                // changes; no event handle, so the call blocks.
                let status = unsafe {
                    RegNotifyChangeKeyValue(key.0.0, false, REG_NOTIFY_CHANGE_LAST_SET, None, false)
                };
                if status != ERROR_SUCCESS {
                    return;
                }
                let now = apps_use_light_theme();
                if now != light {
                    light = now;
                    changed(light);
                }
            }
        })
        .is_ok()
}

/// The key watched by [`watch_light_theme`], moved to its thread.
struct Watched(Key);

// SAFETY: a registry key handle is not tied to the thread that opened it,
// and only the watching thread uses it after the move.
unsafe impl Send for Watched {}

fn read_dword(key: &Key, name: &str) -> Option<u32> {
    let mut value = 0_u32;
    let mut bytes = 4_u32;
    // SAFETY: `value` holds the four bytes a DWORD needs.
    let status = unsafe {
        RegGetValueW(
            key.0,
            None,
            &HSTRING::from(name),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut bytes),
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}
