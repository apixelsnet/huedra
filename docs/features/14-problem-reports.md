# 14 — Problem reports

When Huedra crashes, the next start offers to copy the details for a problem report. **Report a problem...** in the tray menu copies them at any time. Huedra sends nothing itself: the person reads and pastes the details wherever they report the problem.

## Scope
- Crash file `crash.txt` in the local data folder, one entry: time, version, role (tray or settings window) and the reason. Written by the panic hook (Rust panics; release builds abort).
- Next start of the tray or the settings window: the first to move `crash.txt` to `crash.last.txt` asks, in a Windows message box on its own thread, "Huedra closed unexpectedly". **Yes** copies the details and opens the log folder; **No** leaves it.
- The details: Huedra and Windows versions, portable or not, the newest crash, the last 100 log lines. The profile folder (`%USERPROFILE%`) is replaced in any letter case, so the user name is not in them.
- Copy and open: UTF-16 text on the clipboard (a message-only window owns it), Explorer opened with `huedra.log` selected.

## Notes
- Chosen by the owner (2026-11-01): copy the details and open the log rather than a prefilled GitHub issue, so it works without a GitHub account; offered after a crash and from a menu. A Report a problem button under General › About made the exe 10,018,816 bytes, over the 10 MB limit, so the owner moved it to the tray menu (menu items need no Slint code). Still 1,920 bytes over (10,001,920), so the owner dropped the unhandled-exception filter for Windows-level crashes (access violations); panics, the likelier kind, are still recorded.
- Not recorded: Windows exceptions, `__fastfail`, and the process being killed.

## Result (2026-11-01)
Code: `crates/huedra-core/src/report.rs` (crash entry, Windows name, report text, profile redaction), `crates/huedra-win/src/report.rs` (offer, clipboard), `crates/huedra-win/src/shell.rs` (`show_in_folder`), `crates/huedra/src/main.rs` (`record_crashes`), `crates/huedra/src/app.rs` (tray menu item `MENU_REPORT`), `crates/huedra-core/src/locations.rs` (`crash`, `last_crash`).
- Tests: Windows 10 and 11 names, report layout with the newest 100 lines, "None recorded." without a crash, profile hidden in any case but not inside a longer folder name, short logs.
Not verified yet: a real crash and the dialog on the next start, the clipboard and Explorer on this PC.
