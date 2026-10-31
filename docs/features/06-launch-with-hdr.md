# 06 — Launch with HDR

Turns HDR on before a game starts, for games that misbehave if HDR changes during start-up.

## Scope
- `huedra launch -- <command>`: turn HDR on, wait until active, start the game, hold HDR until it exits.
- Steam launch option text, desktop shortcuts, "(with HDR)" context-menu entry.
- Arguments go to `CreateProcessW` directly, never through `cmd.exe`.

## Done when
A Steam game started through `huedra launch` starts in HDR, and every registry entry written is removed by `huedra uninstall`.

## Result (2026-10-31) — phase 4
Code: `crates/huedra-core/src/launch.rs` (trigger ids, wait folder, command texts, shortcut names; tests), `crates/huedra-core/src/command.rs` (`launch`, `uninstall`), `crates/huedra-core/src/ipc.rs` (`Request::Launch`), `crates/huedra-win/src/job.rs`, `crates/huedra-win/src/process.rs` (`spawn_detached`), `crates/huedra-win/src/registry.rs` (menu entry, `remove_all`), `crates/huedra-win/src/shell.rs` (desktop shortcut), `crates/huedra/src/launch.rs`, `crates/huedra/src/app.rs` (`launch`), `crates/huedra-ui` (Games detail, General page).
- `huedra launch [--] <program> [arguments]`: asks the tray to turn HDR on at once (trigger `launch:<pid of huedra launch>`, no start delay, the game's hold time or the default), waits for the answer, then starts the program. The tray holds HDR while the `huedra launch` process lives, so a killed launch still ends the trigger. When no tray runs, `launch` starts one (no console, no inherited handles, outside the caller's job where that job allows it) and waits up to 15 s for it. The program starts even when HDR stays off; the reason is printed (Always SDR, paused, no managed display, switch failed).
- The program is started by `std::process` (`CreateProcessW`, never `cmd.exe`; `.bat` and `.cmd` are refused) suspended, put in a job object with breakaway allowed and no kill-on-close, then resumed. After it exits, `launch` keeps waiting while a process in the job runs from the wait folder: the working folder when the program lies inside it (Steam starts games in their install folder), otherwise the program's folder. Helpers elsewhere on the system (a store app the game opened) do not hold HDR. The program's exit code is passed on (codes outside 0 to 255 become 1).
- The game's name in the log comes from the library when the program belongs to a known game, otherwise from the file name. "Never for" in the tray menu skips launch triggers.
- Settings: a Steam game's detail shows the launch option `"<huedra.exe>" launch -- %command%` with a Copy button; games of other stores with a known program (not Xbox) get "Create a desktop shortcut", which writes `<game> (with HDR).lnk` on the desktop (target Huedra, arguments `launch -- "<program>"`, working folder and icon from the game). General has a toggle for "Start with HDR" in the menu of programs: `HKCU\Software\Classes\exefile\shell\Huedra.StartWithHdr` with `"<huedra.exe>" launch -- "%1"` (under Show more options on Windows 11).
- `huedra uninstall` closes the running tray, removes the start-up value and the menu entry (and the `exefile` keys above it when they are left empty), and names the data folder to delete by hand. Desktop shortcuts are files and stay.

Verified on this PC, with a portable copy in Always SDR mode (no HDR switch) and hidden `powershell.exe` copies as games:
- `launch` with no tray running started the tray, got its answer ("HDR stays off: Huedra is in Always SDR mode.") and passed the exit code 7 on; the log shows "game is starting with HDR." and "game ended (started with HDR).". With the output captured by the caller it returned in 1.7 s (it hung until the tray exited before the auto-started tray stopped inheriting handles).
- A launcher that started the game from its folder and exited at once: `launch` returned after the game (5.3 s), not after the launcher (0.4 s before the job fix). A program outside the folder (`ping.exe`) did not hold it (0.2 s).
- `.bat` and a missing program exit 2 with a message.
- `uninstall`: closed the tray, removed a menu key written as the settings window writes it, left no empty `exefile` keys, and a second run succeeds.

Not verified: a real HDR switch through `launch`; a Steam game with the launch option (whether Steam lets the tray leave its job); the Copy button, the shortcut button and the Explorer menu entry itself. The launch option field and the menu switch were checked later on a hidden desktop (see 08).

## Result (2026-10-31) — One display
Code: `launch` in `crates/huedra/src/app.rs`.
- With Displays, "Only the display showing the content" on (the default), `huedra launch` turns HDR on for the main display only, where games open by default: the program has no window yet to follow. Off, every managed display. A game in the library then also follows its own window (03).

Not verified on this PC: a real launch.

Status: done except the checks above (phase 4).
