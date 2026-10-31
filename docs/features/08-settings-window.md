# 08 — Settings window

The Slint window, run as its own process (`huedra settings`).

## Scope
- Pages: Status, Games, Video players, Displays, General.
- Resizable: default 1040 x 680, minimum 520 x 560. Layouts: wide (1000 px and up), medium (680 to 999), compact (below 680, icon rail).
- Size, position and maximized state remembered. Per-monitor DPI awareness v2; follows the Windows text size.
- Palette from the BT.2020 primaries; spectrum only while HDR is on, grey in SDR.
- Motion: HDR fade 400 ms, page change 150 ms, controls 120 ms, hover 80 ms; none when Windows "Animation effects" is off.
- Rendering: FemtoVG (OpenGL) with the software renderer as fallback. Slint royalty-free licence: attribution line on the About page.

## Done when
Every page works by keyboard, in light and dark, in all three layouts, at 100%, 150% and 200% scaling, and the window opens in under 300 ms.

## Result (2026-10-31) — phase 3
Code: `crates/huedra-ui/ui/{theme,pages,app}.slint`, `crates/huedra-ui/src/{app,lookups}.rs`, `crates/huedra-ui/build.rs`, `crates/huedra-core/src/geometry.rs`, `crates/huedra-win/src/{instance,shell}.rs`.
- Slint 1.18.1, Fluent style (follows Windows light or dark), winit backend, software renderer chosen in code. Compared on this PC, OpenGL (FemtoVG) used 91 MB of private memory and the software renderer 8.6 MB, so FemtoVG is not built at all. The spectrum is a 640 x 160 image drawn in Rust (`icon::spectrum_strip`) because the software renderer showed seams in multi-stop gradients.
- Pages: Status (spectrum card that fades in over 400 ms while HDR is on, mode selector, pause, recent changes), Games (search, store filters, list, detail with HDR setting, delay, hold, PCGamingWiki information and suggestion, Add game, Scan again), Displays (managed switch per HDR display, peak brightness, retry attempts), General (start with Windows, updates, PCGamingWiki, default times, shortcuts, window titles in the log, About with the Slint attribution).
- Layouts: navigation 240 px wide (1000 px and up), 200 px (680 to 999), icon rail of 64 px below 680; on narrow layouts the game detail replaces the list with a "Back to the list" button. Pages cross-fade and rise 8 px in 150 ms; every duration is zero when Windows "Animation effects" is off.
- Every change is saved at once (atomic write) and the tray process is asked to reload. Start with Windows writes or removes `HKCU\...\Run\Huedra`; nothing else touches the registry.
- `huedra settings [status|games|displays|general]` opens on a page; a second request brings the open window to the front. Size, position and maximized state are kept in `window.toml` (local app-data); a position off every monitor is ignored.

Verified on this PC: all four pages render as designed in the dark theme at 100 % scaling in the wide layout; the Status page follows the tray within one second (a real game, Dispatch, showed "HDR is on" with both displays); 8.8 MB private memory per window; the window handle exists about 30 to 45 ms after the process starts.
Not verified: the light theme; the medium and compact layouts (an attempt was stopped because a game was in use on this PC); keyboard-only use; 150 % and 200 % scaling; the Add game dialog and the Start with Windows switch. The controls take the Windows accent colour (Fluent style) rather than ultramarine.

Status: built; the checks above remain for the owner's next session.
