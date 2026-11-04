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

## Result (2026-11-02) — a program that cannot start leaves HDR alone
Code: `Suspended` in `crates/huedra-win/src/job.rs`, `run` in `crates/huedra/src/launch.rs`.
- Found in the end-to-end pass: `huedra launch -- C:\nope\missing.exe` asked the tray for HDR first, then failed to start the program, so the display switched on and back off after the hold time, and the exit code was 1.
- `launch` now creates the program suspended (inside its job) before asking the tray; a program that cannot be created is reported with exit code 2 and HDR is not touched. The program is resumed once the tray has answered, so it still starts with HDR on. A suspended program that is never resumed is ended (drop guard).

Verified on this PC with a portable copy: a missing program exits 2 and both displays stay in SDR; a fake game started through `launch` gets HDR on Display 1 before its window appears (0.7 s) and SDR after it closes and the hold time ends; `launch -- cmd.exe /c "exit 5"` with no tray running starts the tray and passes 5 on; `.bat` still exits 2.

## Result (2026-11-04) — HDR when Steam starts the game
Code: `crates/huedra-core/src/steam_start.rs` (state and tests), `Config::steam_start_trigger` and `GameSettings::steam_start` in `crates/huedra-core/src/config.rs`, `steam_running_app` / `watch_steam_running_app` in `crates/huedra-win/src/registry.rs`, `Event::SteamApp` in `crates/huedra-win/src/event_loop.rs`, `on_steam_app` and the helpers after it in `crates/huedra/src/app.rs`, the Games detail in `crates/huedra-ui`.
- Reported for Battlefield 6: games are found when their window comes to the front, so HDR came on while the game was already starting, and the game grayed out its HDR settings (it checks HDR once at start). The Steam launch option turned HDR on first, but the game's anti-cheat ended the game at once when `huedra launch` started it (created suspended, inside a job, with Huedra as its parent).
- New per-game switch, off by default (owner's choice), for Steam games: **Turn HDR on when Steam starts the game** (`steam_start = true` in the game's `[[game]]` entry). Steam writes the app id it runs to `HKCU\Software\Valve\Steam\RunningAppID` within about a second of starting the game's first program (measured 0.7 s after it, see below), and 0 a moment after the game has ended. The tray watches that value (`RegNotifyChangeKeyValue` on a thread of its own, armed again before each read so a write is not missed; no polling) and, for a game set to Enable while running with the switch on, starts the trigger `steam-start:<app id>` at once: no start delay, the game's hold time, the main display (every managed display when HDR does not follow the content). Nothing opens or touches Steam's or the game's processes.
- The early start follows the display of the game's own window once it is found, and ends when the game's own trigger ends (or "Never for" the game). When Steam stops reporting the game before it ever appeared (a first program that hands over to the EA app, a canceled start), it ends 60 s later unless the game turns up; Steam reporting it again cancels that.
- A game Steam already runs when the tray starts gets no early start. "Never for" in the tray menu skips early starts, as it skips launches.
- The Light/dark theme watcher shares the new registry watch helper; it now also arms before reading.
- Games detail: the Start with HDR section of a Steam game shows the switch above the launch option, which now notes that games with anti-cheat may refuse to start that way.

Verified on this PC with a portable copy in Always SDR mode (no HDR switch) and `RunningAppID` written by a script (restored to 0 afterwards): a Steam game without the switch logged nothing; Hollow Knight: Silksong with the switch logged "Steam is starting Hollow Knight: Silksong; HDR turns on before it runs." in the same second; after the value went back to 0 it ended 60 s later ("did not appear after Steam started it"); the value returning within the grace kept it. Build green on the build server (clippy, tests, docs, release, 10,100,736 bytes), Defender scan clean.

Verified by the owner on this PC (portable copy of this build, Automatic mode, hold 3 s, the switch on for the installed Steam games): Play on Hollow Knight: Silksong. A 100 ms recorder saw the game's program start at 08:54:59.72 and `RunningAppID` change at 08:55:00.46, so Steam writes the value after the first program starts, not before. Huedra logged the early start and turned HDR on for Display 1 in the same second ("Switch requested 2 ms after the event"); the game's own detection (its window in front) followed at 08:55:07, 7 s later, which is how late HDR used to come on. The game ended at 08:55:27, Steam wrote 0 at 08:55:29, and Display 1 returned to SDR at 08:55:30 after the hold.

Not verified: Battlefield 6 itself (not installed here; its Steam program hands over to the EA app and anti-cheat, which leaves more time before its window); the switch in the settings window.
