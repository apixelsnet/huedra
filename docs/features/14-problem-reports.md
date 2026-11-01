# 14 — Problem reports

When Huedra crashes, the next start offers to copy the details for a problem report. **Report a problem...** in the tray menu copies them at any time. Huedra sends nothing itself: the person reads and pastes the details wherever they report the problem.

## Scope
- Crash file `crash.txt` in the local data folder, one entry: time, version, role (tray or settings window) and the reason. Written by the panic hook (Rust panics; release builds abort).
- Next start of the tray or the settings window: the first to move `crash.txt` to `crash.last.txt` asks, in a Windows message box on its own thread, "Huedra closed unexpectedly". **Yes** copies the details; **No** leaves it.
- The details: Huedra version, Windows version and build (`DisplayVersion`, `CurrentBuild`), portable or not, the newest crash (`crash.last.txt`), the last 100 lines of `huedra.log`. Paths inside the profile folder, written as Windows writes it, show as `%USERPROFILE%\...`, so the user name is not in them.
- Copy: UTF-16 text on the clipboard (a message-only window owns it). No Explorer window; the details already hold the newest log lines.

## Notes
- Chosen by the owner (2026-11-01): copy the details rather than a prefilled GitHub issue, so it works without a GitHub account; offered after a crash and from a menu.
- Size (10 MB limit, measured with the update settings built in, as CI builds): a Report a problem button under General › About made the exe 10,018,816 bytes, so the owner moved it to the tray menu (menu items need no Slint code); at 10,001,920 the owner dropped the unhandled-exception filter for Windows-level crashes; at 10,000,896 the report was simplified (no Windows 10/11 naming, exact-case profile match, current log file only). Measured parts: the clipboard code 1,024 bytes, the whole report 3,584 bytes. Final: 9,999,872 bytes, 128 under the limit.
- `clip.exe` instead of the clipboard code was tried and was 1.5 KB larger (the piped child process).
- Not recorded: Windows exceptions (access violations), `__fastfail`, and the process being killed.

## Result (2026-11-01)
Code: `crates/huedra-core/src/report.rs` (crash entry, Windows name, report text, profile redaction), `crates/huedra-win/src/report.rs` (offer, clipboard), `crates/huedra/src/main.rs` (`record_crashes`), `crates/huedra/src/app.rs` (tray menu item `MENU_REPORT`), `crates/huedra-core/src/locations.rs` (`crash`, `last_crash`).
- Tests: Windows name, report layout with the newest 100 lines, "None recorded." without a crash, profile hidden but not inside a longer folder name, short logs.

Verified on this PC (portable test copy, `huedra settings` on a hidden desktop, Yes sent as a window message): the dialog appears with its text and Yes/No; `crash.txt` became `crash.last.txt`; Yes closed it and put the report on the clipboard: "Windows 26H2 (build 26300)", portable yes, the crash line, exactly the last 100 log lines, a game path shown as `%USERPROFILE%\Games\...`. Explorer started for the log folder (on the hidden desktop).

Not verified: the tray menu item **Report a problem...** (same function as Yes), a real panic writing `crash.txt`.

## Result (2026-11-01, Defender)
Defender quarantined this build (`dist\huedra.exe` and the test copy) as `Trojan:Win32/Bearfoos.A!ml`, its machine-learning heuristic (see 12). Scanned on this PC with `MpCmdRun -Scan -ScanType 3 -File`: the build with clipboard copy and Explorer opening the log was flagged; the same build without the clipboard code (report written to a file instead) and the same build without opening Explorer both scanned clean. The owner chose copy only: `copy_details` no longer opens Explorer, the dialog asks "Copy the details for a problem report to the clipboard?", and `shell::show_in_folder` is gone.
- Verified on this PC: the new build (9,999,360 bytes) scanned clean, ran the hidden-desktop test without a detection, showed the new question, claimed the crash and copied the report; no Explorer window started.
- The verdict sits close to the threshold; any build can be flagged again until the exe is signed. Scan each new build before handing it over.
