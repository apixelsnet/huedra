# 07 — Tray and hotkeys

The always-visible part of Huedra.

## Scope
- Tray icon: spectrum dot when HDR is on, grey ring in SDR. The tooltip states the mode and the reason.
- Menu: status line, Automatic / Always HDR / Always SDR, Never for (game), Use SDR for this video, Pause for 1 hour, Open Huedra, Exit.
- `Shell_NotifyIconW` and `TrackPopupMenuEx` written directly; no tray crate.
- Hotkeys via `RegisterHotKey`: Win+Alt+H, Win+Alt+O, Win+Alt+P. Conflicts are reported, never dropped silently.

## Done when
Every menu item and hotkey works on this PC, and the icon follows the state.

Status: not started (phase 2).
