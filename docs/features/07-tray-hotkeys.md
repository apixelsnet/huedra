# 07 — Tray and hotkeys

The always-visible part of Huedra.

## Scope
- Tray icon: spectrum dot when HDR is on, grey ring in SDR. The tooltip states the mode and the reason.
- Menu: status line, Automatic / Always HDR / Always SDR, Never for (game), Use SDR for this video, Pause for 1 hour, Open Huedra, Exit.
- `Shell_NotifyIconW` and `TrackPopupMenuEx` written directly; no tray crate.
- Hotkeys via `RegisterHotKey`: Win+Alt+H, Win+Alt+O, Win+Alt+P. Conflicts are reported, never dropped silently.

## Done when
Every menu item and hotkey works on this PC, and the icon follows the state.

## Result (2026-10-31) — phase 2
Code: `crates/huedra-win/src/{tray,menu,hotkey}.rs`, `crates/huedra-core/src/icon.rs`, `crates/huedra/src/app.rs`, `crates/huedra/build.rs`.
- Icons are drawn in code at the system small-icon size (`icon::render`, 4x4 supersampled): a conic dot through pink, violet, ultramarine, cyan and green while HDR is on, a grey ring in SDR. A test checks that the gradient never produces yellow.
- Tooltip: "Huedra" and "HDR on · Character Map", "SDR · Automatic", "SDR · Paused until 14:42".
- Menu: state line, reason line, Automatic (Win+Alt+O) / Always HDR / Always SDR as radio items, "Never for (game)" while a game runs, Pause for 1 hour / Resume (Win+Alt+P), Open Huedra, Exit. Left click opens the settings window; the icon is added again when Explorer restarts (`TaskbarCreated`).
- Shortcuts Win+Alt+H (HDR on or off), Win+Alt+O (next mode), Win+Alt+P (pause or resume); refused shortcuts are written to the log. The setting `hotkeys = false` turns them off.
- The executable carries version information and an icon (the HDR dot at 16 to 256 px), built by `build.rs` with `llvm-rc`.

Verified on this PC: all three shortcuts (synthetic key presses) change the state as described; tooltips read back through `huedra status`.
Not verified: the menu and icon were not inspected by eye during this session; contrast of the grey ring on light and dark taskbars.

Status: done; "Use SDR for this video" comes with video in phase 4.

## Result (2026-11-01) — phase 5
- Menu item "Install Huedra x.y.z" while a release waits for consent (Updates: Ask before installing); it starts `huedra update`, which closes the tray, installs and starts the new version (see 11).
- The log's start line gives the time from process creation to the tray icon, and the game library scan time: "Huedra 0.1.0 started in 34 ms; game libraries read in 3 ms." Each switch logs "Switch requested N ms after the event".
- First start (no settings file, not portable) adds the start-up entry, since Start with Windows is on by default; an entry naming a program that no longer exists is pointed at the running one.

Verified on this PC: start lines from the running copy (34 ms, 3 ms) and from test copies (44–46 ms warm; 119 ms and 91 ms on the very first run of a newly copied exe). Not verified: the update menu item (needs a release address); the switch timing line during a real switch.

## Result (2026-11-01) — slow tray menu
Code: `window_proc` in `crates/huedra-win/src/event_loop.rs`.
- Reported by the owner: right-clicking the icon took a long time to show the menu.
- Cause: the window procedure only queued events, counting on the loop to drain the queue after `DispatchMessageW`. Explorer's tray callbacks (and `WM_DISPLAYCHANGE`) are sent, not posted, so `GetMessageW` handled them inside its wait without returning; the queued menu event waited for the next posted message (a timer, a window coming to the front, a pipe call). Left-click (opening the settings window) and display changes were delayed the same way.
- Fix: every queued event also posts a `WM_NULL` to the tray window, so `GetMessageW` returns at once.

Verified: build green; the fixed build runs as the owner's tray. Verified by the owner (2026-11-01): several right-clicks on the icon, the menu shows almost at once. Not verified: left-click timing and display-change reaction were not timed separately (same code path).

## Result (2026-11-01) — gamut icon
Code: `crates/huedra-core/src/icon.rs`.
- The icon is now the visible colour gamut (CIE 1931 spectral locus, drawn a quarter wider than its true proportions): in HDR filled with the BT.2020 palette round the edge (green at the top, pink to the right, violet and ultramarine below, cyan on the left, no yellow) with a white glow at the white point and the white BT.2020 triangle from 32 pixels up; in SDR a grey outline of the same shape. The same `render` draws the tray icon, the program icon, the settings window's logo and the extension icons.
- Tests: shape filled and outline hollow at the white point, corners clear, green on top and pink on the right, no yellow anywhere in the palette sweep.
Verified: unit tests in CI; a preview of the same drawing at 16 to 256 pixels on dark and light backgrounds; the 16-pixel SDR outline is still recognisable. Not verified: the tray icon in the owner's own notification area (the new build is not installed there yet).

## Result (2026-11-01) — Ko-fi menu item
Code: `crates/huedra/src/app.rs` (`MENU_SUPPORT`).
- Menu item **Support Huedra on Ko-fi...** between Open Huedra and Exit; opens `update::SUPPORT_URL` with `shell::open_web_page` (https only) and logs a line if the browser cannot be started.

Verified: CI green (a84f210). Verified by the owner on this PC (2026-11-01): the item opens the Ko-fi page in the browser. Not verified: the log line when no browser can be started.

## Result (2026-11-01) — the window on start
Code: `TrayStart`, `BACKGROUND`, `startup_command`, `startup_program` in `crates/huedra-core/src/command.rs`; `run`, `startup_entry` in `crates/huedra/src/app.rs`; `open_settings` in `crates/huedra/src/main.rs`; `pass_on_foreground` in `crates/huedra-win/src/instance.rs`.
- `huedra` without arguments starts the tray and opens the settings window. `huedra --background` starts the tray only; the start-up entry (`"<exe>" --background`), an update's restart, a rollback, `launch` and the disclaimer's Accept use it.
- A second start of `huedra` opens the window of the running copy, or brings the open one to the front (restored when minimised); a second `--background` start does nothing. Before it starts the window process, the tray passes on its right to come to the front (`AllowSetForegroundWindow(ASFW_ANY)`), which it holds when the user started it or clicked its icon.
- The tray rewrites a start-up entry for its own program that lacks `--background` (entries from 0.1.0), so signing in does not open the window. Copies that update from 0.1.0 are restarted by 0.1.0's updater without the argument and open the window once.

Verified on this PC with the build of this change (portable copy): a plain start ran the tray and opened the window; a second start with the window minimised restored it, with one tray and one window process; closing the window kept the tray; a `--background` start while it ran opened nothing. Unit tests for the arguments and the start-up command.
Verified by the owner on this PC (2026-11-01): with the window minimised, starting `huedra.exe` again from File Explorer brought it to the front.
Not verified: the start-up entry and its rewrite (the copy here is portable and writes no entry); sign-in with the entry.

## Result (2026-11-01) — Exit closes the window
Code: `MENU_EXIT` in `crates/huedra/src/app.rs`; `close_window` in `crates/huedra-win/src/instance.rs`.
- Exit in the tray menu closes the settings window as well (`WM_CLOSE` to the window titled `APP_NAME`, as its close button does), then the tray. `huedra exit` and the updater's restart leave the window open, so the window that started an update can show its result.

Verified: build green. Verified by the owner on this PC (2026-11-02, test build): with the window open, Exit in the tray menu closed the window and the tray.

## Result (2026-11-02) — Buy Me a Coffee
Code: `update::SUPPORT_URL` in `crates/huedra-core/src/update.rs`, `MENU_SUPPORT` in `crates/huedra/src/app.rs`.
- The support item is now **Support Huedra on Buy Me a Coffee...** and opens https://buymeacoffee.com/pixelsthecoder (owner's choice, 2026-11-02), replacing Ko-fi.
