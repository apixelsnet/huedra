//! Problem reports: the line Huedra writes when it stops after an error, and
//! the details a person can copy and send with a problem report. Huedra itself
//! sends nothing. Feature doc: `docs/features/14-problem-reports.md`.

use crate::{APP_NAME, VERSION};

/// How many of the newest log lines a report includes.
pub const LOG_LINES: usize = 100;

/// Windows 11 builds start here; earlier ones are Windows 10.
const FIRST_WINDOWS_11_BUILD: u32 = 22_000;

/// Put in place of the user's profile folder, so a report does not carry
/// the user name.
const PROFILE_PLACEHOLDER: &str = "%USERPROFILE%";

/// The crash file's content: when, which part of Huedra, and why.
pub fn crash_entry(time: &str, role: &str, message: &str) -> String {
    format!("{time} {APP_NAME} {VERSION} ({role}) stopped: {message}\n")
}

/// "Windows 11 24H2 (build 26100)", from the registry's `CurrentBuild` and
/// `DisplayVersion`.
pub fn windows_name(build: Option<&str>, display_version: Option<&str>) -> String {
    let number = build.and_then(|b| b.trim().parse::<u32>().ok());
    let name = match number {
        Some(n) if n >= FIRST_WINDOWS_11_BUILD => "Windows 11",
        Some(_) => "Windows 10",
        None => "Windows",
    };
    let version = display_version
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| format!(" {v}"))
        .unwrap_or_default();
    let build = number.map(|n| format!(" (build {n})")).unwrap_or_default();
    format!("{name}{version}{build}")
}

/// What goes into a report.
#[derive(Debug, Clone, Copy)]
pub struct Details<'a> {
    /// From [`windows_name`].
    pub windows: &'a str,
    /// Whether this is a portable copy.
    pub portable: bool,
    /// The newest crash file's content, if there is one.
    pub crash: Option<&'a str>,
    /// The log, oldest line first (the rotated file followed by the current one).
    pub log: &'a str,
    /// The user's profile folder, replaced by `%USERPROFILE%`.
    pub profile: Option<&'a str>,
}

/// The report text: versions, the last crash and the newest log lines.
pub fn text(details: &Details<'_>) -> String {
    let crash = details
        .crash
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .unwrap_or("None recorded.");
    let portable = if details.portable { "yes" } else { "no" };
    let report = format!(
        "{APP_NAME} {VERSION} problem report\n\
         Windows: {}\n\
         Portable copy: {portable}\n\
         \n\
         Last crash:\n{crash}\n\
         \n\
         Last {LOG_LINES} log lines:\n{}\n",
        details.windows,
        last_lines(details.log, LOG_LINES),
    );
    match details.profile {
        Some(profile) => redact(&report, profile),
        None => report,
    }
}

/// The last `count` lines of `text`, without a trailing line break.
fn last_lines(text: &str, count: usize) -> &str {
    let text = text.trim_end();
    let start = text
        .rmatch_indices('\n')
        .nth(count.saturating_sub(1))
        .map_or(0, |(i, _)| i + 1);
    &text[start..]
}

/// Replaces the profile folder (in any letter case) with `%USERPROFILE%`,
/// only where the whole folder name matches: `C:\Users\Al` is not replaced
/// inside `C:\Users\Alex`.
fn redact(text: &str, profile: &str) -> String {
    let profile = profile.trim_end_matches(['\\', '/']);
    if profile.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(found) = find_ignore_case(rest, profile) {
        let end = found + profile.len();
        let whole = rest[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.');
        out.push_str(&rest[..found]);
        out.push_str(if whole {
            PROFILE_PLACEHOLDER
        } else {
            &rest[found..end]
        });
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The byte position of `needle` in `haystack`, ignoring ASCII letter case.
fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack.char_indices().map(|(i, _)| i).find(|&i| {
        haystack
            .get(i..i + needle.len())
            .is_some_and(|part| part.eq_ignore_ascii_case(needle))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_names() {
        assert_eq!(
            windows_name(Some("26100"), Some("24H2")),
            "Windows 11 24H2 (build 26100)",
            "Windows 11"
        );
        assert_eq!(
            windows_name(Some("19045"), Some("22H2")),
            "Windows 10 22H2 (build 19045)",
            "Windows 10"
        );
        assert_eq!(windows_name(None, None), "Windows", "unknown");
    }

    #[test]
    fn report_has_versions_crash_and_newest_lines() {
        let log: String = (1..=150).map(|n| format!("line {n}\n")).collect();
        let crash = crash_entry("2026-11-01 18:06:04", "tray", "boom");
        let report = text(&Details {
            windows: "Windows 11 24H2 (build 26100)",
            portable: false,
            crash: Some(&crash),
            log: &log,
            profile: None,
        });
        assert!(
            report.starts_with(&format!("Huedra {VERSION} problem report\n")),
            "title: {report}"
        );
        assert!(report.contains("Windows: Windows 11 24H2"), "windows");
        assert!(report.contains("(tray) stopped: boom"), "crash");
        assert!(report.contains("\nline 51\n"), "first kept line");
        assert!(!report.contains("\nline 50\n"), "older lines left out");
        assert!(report.trim_end().ends_with("line 150"), "newest line last");
    }

    #[test]
    fn no_crash_is_said_plainly() {
        let report = text(&Details {
            windows: "Windows",
            portable: true,
            crash: None,
            log: "",
            profile: None,
        });
        assert!(report.contains("Last crash:\nNone recorded."), "{report}");
        assert!(report.contains("Portable copy: yes"), "{report}");
    }

    #[test]
    fn profile_folder_is_hidden_in_any_case() {
        let report = text(&Details {
            windows: "Windows",
            portable: false,
            crash: Some(r"panicked at C:\Users\Alex\src"),
            log: "Game: c:\\users\\alex\\Games\\a.exe\nOther: C:\\Users\\Alexandra\\b.exe\n",
            profile: Some(r"C:\Users\Alex\"),
        });
        assert!(!report.contains(r"\Alex\"), "name hidden: {report}");
        assert!(!report.contains(r"\alex\"), "lower case hidden: {report}");
        assert!(
            report.contains(r"%USERPROFILE%\Games\a.exe"),
            "replaced: {report}"
        );
        assert!(
            report.contains(r"C:\Users\Alexandra\b.exe"),
            "a longer name is another folder: {report}"
        );
    }

    #[test]
    fn last_lines_of_short_logs() {
        assert_eq!(last_lines("a\nb\n", 5), "a\nb", "all lines");
        assert_eq!(last_lines("a\nb\nc", 2), "b\nc", "two newest");
        assert_eq!(last_lines("", 3), "", "empty");
    }
}
