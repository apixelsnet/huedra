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

Status: not started (phase 3).
