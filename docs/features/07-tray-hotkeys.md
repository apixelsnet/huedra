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
