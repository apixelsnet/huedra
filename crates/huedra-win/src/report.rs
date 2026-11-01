//! Problem reports on Windows: offering on the next start after a crash
//! to copy the details, and copying them. Huedra sends nothing itself; the
//! person pastes the details wherever they report the problem.
//! Feature doc: `docs/features/14-problem-reports.md`.

use std::fs;
use std::io::Write as _;
use std::os::windows::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use huedra_core::locations::Locations;
use huedra_core::report::{self, Details};
use windows::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW};
use windows::core::HSTRING;

use crate::registry::{self, Hive};
use crate::shell;

const VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
/// `CREATE_NO_WINDOW`: no console window for `clip.exe`.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const OFFER_TITLE: &str = "Huedra closed unexpectedly";
const OFFER_TEXT: &str = "Huedra stopped after an error last time.\n\n\
    Copy the details for a problem report and open the log folder? \
    Huedra sends nothing itself.";

/// When the last run crashed, asks once whether to copy the details. The
/// question waits on its own thread, so the caller goes on starting.
pub fn offer_after_crash(locations: &Locations) {
    // Moving the file claims the crash, so the tray and the settings window
    // never both ask about it.
    if fs::rename(&locations.crash, &locations.last_crash).is_err() {
        return;
    }
    let locations = locations.clone();
    std::thread::spawn(move || {
        // SAFETY: a modal message box with no owner; both strings outlive it.
        let answer = unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(OFFER_TEXT),
                &HSTRING::from(OFFER_TITLE),
                MB_YESNO | MB_ICONWARNING,
            )
        };
        if answer == IDYES {
            copy_details(&locations);
        }
    });
}

/// Copies the problem report to the clipboard and shows the log in
/// Explorer. Returns whether the copy worked.
pub fn copy_details(locations: &Locations) -> bool {
    let copied = copy_text(&details(locations));
    shell::show_in_folder(&locations.log);
    copied
}

/// The report: versions, the newest crash and the newest log lines, with
/// the profile folder hidden.
fn details(locations: &Locations) -> String {
    let windows = report::windows_name(
        registry::string(Hive::LocalMachine, VERSION_KEY, "CurrentBuild").as_deref(),
        registry::string(Hive::LocalMachine, VERSION_KEY, "DisplayVersion").as_deref(),
    );
    let crash = fs::read_to_string(&locations.crash)
        .or_else(|_| fs::read_to_string(&locations.last_crash))
        .ok();
    let mut rotated = locations.log.clone().into_os_string();
    rotated.push(".1");
    let read = |path: PathBuf| {
        fs::read(path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    };
    let log = read(PathBuf::from(rotated)) + &read(locations.log.clone());
    let profile = std::env::var("USERPROFILE").ok();
    report::text(&Details {
        windows: &windows,
        portable: locations.portable,
        crash: crash.as_deref(),
        log: &log,
        profile: profile.as_deref(),
    })
}

/// Puts `text` on the clipboard with Windows' own `clip.exe` (from System32,
/// never from the search path).
fn copy_text(text: &str) -> bool {
    let Some(windows) = std::env::var_os("SystemRoot") else {
        return false;
    };
    let clip = PathBuf::from(windows).join(r"System32\clip.exe");
    let Ok(mut child) = Command::new(clip)
        .stdin(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    else {
        return false;
    };
    // UTF-16 with a byte order mark, so clip.exe keeps every character.
    let bytes: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    // Dropping the pipe after writing ends clip.exe's input.
    let written = child
        .stdin
        .take()
        .is_some_and(|mut input| input.write_all(&bytes).is_ok());
    child.wait().is_ok_and(|status| status.success()) && written
}
