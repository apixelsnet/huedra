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

Verified: build green; the fixed build runs as the owner's tray. Not verified: the menu's speed by the owner's own right-click (no synthetic clicks on the owner's taskbar).
