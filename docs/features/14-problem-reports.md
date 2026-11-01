# 14 — Problem reports

When Huedra crashes, the next start offers to copy the details for a problem report. **Report a problem...** in the tray menu copies them at any time. Huedra sends nothing itself: the person reads and pastes the details wherever they report the problem.

## Scope
- Crash file `crash.txt` in the local data folder, one entry: time, version, role (tray or settings window) and the reason. Written by the panic hook (Rust panics; release builds abort).
- Next start of the tray or the settings window: the first to move `crash.txt` to `crash.last.txt` asks, in a Windows message box on its own thread, "Huedra closed unexpectedly". **Yes** copies the details and opens the log folder; **No** leaves it.
- The details: Huedra version, Windows version and build (`DisplayVersion`, `CurrentBuild`), portable or not, the newest crash (`crash.last.txt`), the last 100 lines of `huedra.log`. Paths inside the profile folder, written as Windows writes it, show as `%USERPROFILE%\...`, so the user name is not in them.
- Copy and open: UTF-16 text on the clipboard (a message-only window owns it), Explorer opened with `huedra.log` selected.

## Notes
- Chosen by the owner (2026-11-01): copy the details and open the log rather than a prefilled GitHub issue, so it works without a GitHub account; offered after a crash and from a menu.
- Size (10 MB limit, measured with the update settings built in, as CI builds): a Report a problem button under General › About made the exe 10,018,816 bytes, so the owner moved it to the tray menu (menu items need no Slint code); at 10,001,920 the owner dropped the unhandled-exception filter for Windows-level crashes; at 10,000,896 the report was simplified (no Windows 10/11 naming, exact-case profile match, current log file only). Measured parts: the clipboard code 1,024 bytes, the whole report 3,584 bytes. Final: 9,999,872 bytes, 128 under the limit.
- `clip.exe` instead of the clipboard code was tried and was 1.5 KB larger (the piped child process).
- Not recorded: Windows exceptions (access violations), `__fastfail`, and the process being killed.

## Result (2026-11-01)
Code: `crates/huedra-core/src/report.rs` (crash entry, Windows name, report text, profile redaction), `crates/huedra-win/src/report.rs` (offer, clipboard), `crates/huedra-win/src/shell.rs` (`show_in_folder`), `crates/huedra/src/main.rs` (`record_crashes`), `crates/huedra/src/app.rs` (tray menu item `MENU_REPORT`), `crates/huedra-core/src/locations.rs` (`crash`, `last_crash`).
- Tests: Windows name, report layout with the newest 100 lines, "None recorded." without a crash, profile hidden but not inside a longer folder name, short logs.

Verified on this PC (portable test copy, `huedra settings` on a hidden desktop, Yes sent as a window message): the dialog appears with its text and Yes/No; `crash.txt` became `crash.last.txt`; Yes closed it and put the report on the clipboard: "Windows 26H2 (build 26300)", portable yes, the crash line, exactly the last 100 log lines, a game path shown as `%USERPROFILE%\Games\...`. Explorer started for the log folder (on the hidden desktop).

Not verified: the tray menu item **Report a problem...** (same function as Yes), Explorer showing the folder on the normal desktop, a real panic writing `crash.txt`.
