//! Registry access: reading store keys, the start-up entry, and the Windows
//! light or dark app setting.
//! Feature docs: `04-game-library.md`, `08-settings-window.md`, `12-packaging.md`.

use huedra_core::library::registry::RegistryEntry;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, KEY_WOW64_32KEY,
    REG_NOTIFY_CHANGE_LAST_SET, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    RegCloseKey, RegDeleteKeyValueW, RegEnumKeyExW, RegGetValueW, RegNotifyChangeKeyValue,
    RegOpenKeyExW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PWSTR};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Huedra";
const THEME_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const THEME_VALUE: &str = "AppsUseLightTheme";
const MAX_SUBKEYS: u32 = 4096;
const MAX_STRING_BYTES: u32 = 64 * 1024;

/// Which part of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    /// `HKEY_CURRENT_USER`.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE`, 32-bit view (where the game stores write).
    LocalMachine32,
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
    let mut entries = Vec::new();
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
        let key_name = String::from_utf16_lossy(&name[..len as usize]);
        let Some(child) = open(hive, &format!(r"{path}\{key_name}")) else {
            continue;
        };
        let values = names
            .iter()
            .filter_map(|n| read_string(&child, n).map(|v| ((*n).to_owned(), v)))
            .collect();
        entries.push(RegistryEntry {
            key: key_name,
            values,
        });
    }
    entries
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
    let key = HSTRING::from(RUN_KEY);
    let value = HSTRING::from(RUN_VALUE);
    let status = match command {
        Some(command) => {
            let data: Vec<u16> = command.encode_utf16().chain([0]).collect();
            let bytes = u32::try_from(data.len() * 2).unwrap_or(u32::MAX);
            // SAFETY: `data` is a null-terminated UTF-16 string of `bytes` bytes.
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    &key,
                    &value,
                    REG_SZ.0,
                    Some(data.as_ptr().cast()),
                    bytes,
                )
            }
        }
        // SAFETY: deletes one value of the current user's Run key.
        None => match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &key, &value) } {
            ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
            other => other,
        },
    };
    status.ok()
}

/// Whether Windows asks apps for their light appearance (Settings >
/// Personalisation > Colours). Light when the value is missing, as Windows
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
