# 07 — Tray and hotkeys

The always-visible part of Huedra.

## Scope
- Tray icon: spectrum dot when HDR is on, gray ring in SDR. The tooltip states the mode and the reason.
- Menu: status line, Automatic / Always HDR / Always SDR, Never for (game), Use SDR for this video, Pause for 1 hour, Open Huedra, Exit.
- `Shell_NotifyIconW` and `TrackPopupMenuEx` written directly; no tray crate.
- Hotkeys via `RegisterHotKey`: Win+Alt+H, Win+Alt+O, Win+Alt+P. Conflicts are reported, never dropped silently.

## Done when
Every menu item and hotkey works on this PC, and the icon follows the state.

## Result (2026-10-31) — phase 2
Code: `crates/huedra-win/src/{tray,menu,hotkey}.rs`, `crates/huedra-core/src/icon.rs`, `crates/huedra/src/app.rs`, `crates/huedra/build.rs`.
- Icons are drawn in code at the system small-icon size (`icon::render`, 4x4 supersampled): a conic dot through pink, violet, ultramarine, cyan and green while HDR is on, a gray ring in SDR. A test checks that the gradient never produces yellow.
- Tooltip: "Huedra" and "HDR on · Character Map", "SDR · Automatic", "SDR · Paused until 14:42".
- Menu: state line, reason line, Automatic (Win+Alt+O) / Always HDR / Always SDR as radio items, "Never for (game)" while a game runs, Pause for 1 hour / Resume (Win+Alt+P), Open Huedra, Exit. Left click opens the settings window; the icon is added again when Explorer restarts (`TaskbarCreated`).
- Shortcuts Win+Alt+H (HDR on or off), Win+Alt+O (next mode), Win+Alt+P (pause or resume); refused shortcuts are written to the log. The setting `hotkeys = false` turns them off.
- The executable carries version information and an icon (the HDR dot at 16 to 256 px), built by `build.rs` with `llvm-rc`.

Verified on this PC: all three shortcuts (synthetic key presses) change the state as described; tooltips read back through `huedra status`.
Not verified: the menu and icon were not inspected by eye during this session; contrast of the gray ring on light and dark taskbars.

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
- The icon is now the visible color gamut (CIE 1931 spectral locus, drawn a quarter wider than its true proportions): in HDR filled with the BT.2020 palette round the edge (green at the top, pink to the right, violet and ultramarine below, cyan on the left, no yellow) with a white glow at the white point and the white BT.2020 triangle from 32 pixels up; in SDR a gray outline of the same shape. The same `render` draws the tray icon, the program icon, the settings window's logo and the extension icons.
- Tests: shape filled and outline hollow at the white point, corners clear, green on top and pink on the right, no yellow anywhere in the palette sweep.
Verified: unit tests in CI; a preview of the same drawing at 16 to 256 pixels on dark and light backgrounds; the 16-pixel SDR outline is still recognizable. Not verified: the tray icon in the owner's own notification area (the new build is not installed there yet).

## Result (2026-11-01) — Ko-fi menu item
Code: `crates/huedra/src/app.rs` (`MENU_SUPPORT`).
- Menu item **Support Huedra on Ko-fi...** between Open Huedra and Exit; opens `update::SUPPORT_URL` with `shell::open_web_page` (https only) and logs a line if the browser cannot be started.

Verified: CI green (a84f210). Verified by the owner on this PC (2026-11-01): the item opens the Ko-fi page in the browser. Not verified: the log line when no browser can be started.

## Result (2026-11-01) — the window on start
Code: `TrayStart`, `BACKGROUND`, `startup_command`, `startup_program` in `crates/huedra-core/src/command.rs`; `run`, `startup_entry` in `crates/huedra/src/app.rs`; `open_settings` in `crates/huedra/src/main.rs`; `pass_on_foreground` in `crates/huedra-win/src/instance.rs`.
- `huedra` without arguments starts the tray and opens the settings window. `huedra --background` starts the tray only; the start-up entry (`"<exe>" --background`), an update's restart, a rollback, `launch` and the disclaimer's Accept use it.
- A second start of `huedra` opens the window of the running copy, or brings the open one to the front (restored when minimized); a second `--background` start does nothing. Before it starts the window process, the tray passes on its right to come to the front (`AllowSetForegroundWindow(ASFW_ANY)`), which it holds when the user started it or clicked its icon.
- The tray rewrites a start-up entry for its own program that lacks `--background` (entries from 0.1.0), so signing in does not open the window. Copies that update from 0.1.0 are restarted by 0.1.0's updater without the argument and open the window once.

Verified on this PC with the build of this change (portable copy): a plain start ran the tray and opened the window; a second start with the window minimized restored it, with one tray and one window process; closing the window kept the tray; a `--background` start while it ran opened nothing. Unit tests for the arguments and the start-up command.
Verified by the owner on this PC (2026-11-01): with the window minimized, starting `huedra.exe` again from File Explorer brought it to the front.
Not verified: the start-up entry and its rewrite (the copy here is portable and writes no entry); sign-in with the entry.

## Result (2026-11-01) — Exit closes the window
Code: `MENU_EXIT` in `crates/huedra/src/app.rs`; `close_window` in `crates/huedra-win/src/instance.rs`.
- Exit in the tray menu closes the settings window as well (`WM_CLOSE` to the window titled `APP_NAME`, as its close button does), then the tray. `huedra exit` and the updater's restart leave the window open, so the window that started an update can show its result.

Verified: build green. Verified by the owner on this PC (2026-11-02, test build): with the window open, Exit in the tray menu closed the window and the tray.

## Result (2026-11-02) — Buy Me a Coffee
Code: `update::SUPPORT_URL` in `crates/huedra-core/src/update.rs`, `MENU_SUPPORT` in `crates/huedra/src/app.rs`.
- The support item is now **Support Huedra on Buy Me a Coffee...** and opens https://buymeacoffee.com/pixelsthecoder (owner's choice, 2026-11-02), replacing Ko-fi.

## Result (2026-11-02) — Closing closes everything
Code: `close_with_tray`, `start_status_polling` and the window claim in `crates/huedra-ui/src/app.rs`; `own_windows`, `close_window`, `claim_window` in `crates/huedra-win/src/instance.rs`.
- Once the window has seen the tray answer, it closes itself when the tray's pipe is gone (`PipeError::NotRunning`; a busy or failed call does not count), whatever ended the tray: Exit, `huedra exit`, the updater, a crash or Task Manager. The window running Download and install stays. A window opened while the tray was not running stays open for Start Huedra.
- One window only: the process showing it holds the named mutex `Local\Huedra-settings-window`; a second start waits up to 10 s for that window and brings it to the front. The claim is released before the window reopens after an update.
- Windows are found by title and by program name (`huedra.exe`, `huedra.*.exe`, any folder), not by title alone, so an Explorer window on a folder named Huedra is neither closed nor brought forward. Exit closes every match, not the first.

Verified: build green (fmt, clippy, tests, release), Defender scan clean. On this PC with the test build: three `huedra settings` started at once gave one window; `huedra exit` left no Huedra process; killing the tray process closed the window within 3 s; a window opened with no tray stayed, and closed on `huedra exit` after Start Huedra.

## Result (2026-11-07) — quick panel (UI review, step 3)
Plan: the UI review artifact, flyout option A (owner, 2026-11-05). Decided 2026-11-07: a display's switch in the panel turns HDR on or off on that display now, as Win+Alt+H does for all of them, and the next game or video that starts or ends takes over again.
Code: `crates/huedra-ui/ui/flyout.slint` (`FlyoutWindow`), `Marquee` and `FlyButton` in `controls.slint`, `crates/huedra-ui/src/flyout.rs`; `crates/huedra-core/src/flyout.rs` (`place`, `Rect`), `Engine::set_display` in `engine.rs`, `Request::SetDisplayHdr`, `Request::NeverForActive` and `StatusReport::never` in `ipc.rs`, `Command::Flyout`; `crates/huedra-win/src/monitor.rs`, `Tray::rect` in `tray.rs`; `open_flyout` in `crates/huedra/src/main.rs`, the tray's click and requests in `crates/huedra/src/app.rs`.
- A left click on the tray icon (or Enter on it) opens the quick panel instead of the window; the right button keeps the menu, which stays the full keyboard and screen reader path. The panel is its own process (`huedra flyout left top right bottom`, the icon's rectangle from `Shell_NotifyIconGetRect`), like the settings window.
- Contents, as option A: the logo and "HDR on" (pink) or "SDR" with "Cyberpunk 2077 · on since 21:14" or the reason; a gear for Open Huedra; the mode; **Displays**: each HDR display with "HDR · controlled by Huedra" and its switch, displays Huedra does not control listed without a switch, displays without HDR left out; **Pause 1 hour** / **Resume**; **Never for <game>** while one is active (the same as the menu item); a footer with **Open Huedra** and **Report a problem** (copies the report, as the menu item does).
- Long text fades out at the edge and slides into view while pointed at (four-second loop through `animation-tick()`); with Windows animation effects off it wraps instead. The whole text is the accessible name.
- Window: no frame, no taskbar button, always on top, rounded corners and the system shadow (winit attributes through Slint's `unstable-winit-030` hooks; no new unsafe code there). 360 effective pixels wide at the monitor's scale; placed by `flyout::place` beside the icon on whichever edge the taskbar sits, inside the work area with a 12 px margin, above the icon when it is in the overflow area; at most the work area's height, the display list scrolling beyond it; moved again when its height changes. Closes on losing the focus (after having had it; the process lingers 400 ms so the click on the icon that took the focus does not reopen it), on Escape, on Alt+F4, after Open Huedra, and when the tray ends. A click on the icon while it is open closes it.
- Engine: `set_display` turns one display on (owned by the user, never turned off automatically) or off (silencing the triggers that cover it); a mode that would undo it gives way to Automatic. Two tests.

Verified on this PC: unit tests for placement on all four taskbar edges, the overflow area, a screen smaller than the panel and the command line, and for `set_display`; CI green (135 tests); exe 11,108,352 bytes; Defender clean. The sample build on a hidden desktop in dark and light (mockup layout; Always HDR; Display 2's switch turned it on; Never for Cyberpunk 2077 went to SDR and the button left; Alt+F4 ends the process). With the new build as the tray: a click posted to the tray icon started `huedra flyout 2317 1438 2349 1486`, the panel opened at 2153,1079 to 2513,1426 (centered on the icon, its edge 12 px above the taskbar), and the panel showed both displays of this PC with their switches through the real tray. A second posted click closed the open panel. With this PC's taskbar hiding itself, the icon lies inside the work area and the panel opened just above it (2153,1033 to 2513,1380 for the icon at 2317,1392). The owner's own copy was started again afterwards.
- A test build carrying the update key reports version 0.1.0, so a tray of it with **Install automatically** replaced itself with the released 1.1.2 within minutes: test the tray with a build made without the key.
Not verified: a real mouse click. A posted click carries no right to take the focus; the panel got it in one run of four, so light dismiss (closing when another window takes the focus) was not seen. Also not verified: the switches against real displays (they would switch the owner's monitors), other scales and monitor layouts, Narrator.

## Result (2026-11-07) — a notice when a switch fails (UI review, idea 8c)
Decided 2026-11-07 (the plan's smaller ideas): the failure notice yes, on by default; settings search and customizable Status cards no, as the plan recommended; Jump List tasks and first-run setup deferred.
Code: `Tray::notify` in `crates/huedra-win/src/tray.rs` (`NIF_INFO`, `NIIF_USER | NIIF_RESPECT_QUIET_TIME`), `Event::NoticeClick` (`NIN_BALLOONUSERCLICK`) in `event_loop.rs`, `record` in `crates/huedra/src/app.rs`, `general.notify_failures` in `config.rs`, Settings › Notify me when HDR does not switch.
- When a switch fails after the retries, the tray icon shows a Windows notification: "Display 2 · … did not switch", "Windows did not switch the display. Huedra tried 2 more times. Recent changes on the Status page have the details." Normal switches never notify; several displays failing for one reason make one notice. Windows holds it during quiet hours or a full-screen game and keeps it in the notification center. A click on it opens the window, where the failure notice is on Status.
Verified on this PC with a throwaway build that sent the notice when the tray started (not committed): the notification showed with Huedra's icon, title and text. Not verified: a real failed switch, the click on the notice, quiet hours.

## Result (2026-11-07) — quick panel after the simpler wording
Code: `actions` in `crates/huedra-ui/ui/flyout.slint`, the display lines in `show` in `crates/huedra-ui/src/flyout.rs`.
- **Never use HDR for (game)** (main's new name) has a row of its own under **Pause 1 hour**, so the game's name fits; it was cut off beside Pause. Each display reads "HDR on · controlled by Huedra" or "HDR off · …".

Verified on this PC with the sample build on a hidden desktop: Automatic with Cyberpunk 2077 running shows both rows in full; Always HDR, Display 2's switch, Never use HDR for, light theme and Escape.
Not verified: on the real tray (the merged build was not run as the tray).

## Result (2026-11-14) — pause length and custom shortcuts
Code: `crates/huedra-core/src/shortcut.rs`, `crates/huedra-win/src/hotkey.rs`, `MenuEntry::Submenu` in `crates/huedra-win/src/menu.rs`, `on_menu`, `register_hotkeys`, `keys_of` in `crates/huedra/src/app.rs`, the Keyboard shortcuts section in `crates/huedra-ui/ui/pages.slint`.
- The tray menu's **Pause automatic switching** opens a submenu: 15 minutes, 30 minutes, 1 hour (the shortcut's length), 2 hours, 4 hours. While paused the item is **Resume automatic switching**. The quick panel and Status keep their one-hour Pause.
- Shortcuts are settings: `[general.shortcuts]` with `toggle_hdr`, `cycle_mode` and `pause`, written only when one differs from Win+Alt+H, Win+Alt+O, Win+Alt+P. Text such as `ctrl + alt + f9` is stored tidied (`Ctrl+Alt+F9`); an empty text turns that shortcut off; text that is not a shortcut (typed into the file) is kept as written, logged, and the default is used. A shortcut needs Win, Ctrl or Alt, except F13 to F24; Win+Alt+B (Windows' HDR switch), Win+L and Ctrl+Alt+Delete are refused.
- Settings › Keyboard shortcuts has a box per action; a refused text leaves the old keys and says why. Keys Windows refused because another program holds them come back from the tray in the status report (`shortcut_note`) and show in the section. The menu's shortcut column, Status's key labels and the setup text follow the settings.

Verified on this PC (portable test copy, default keys free after the owner's tray was closed): the submenu lists the five lengths; 15 minutes paused until 15 minutes later ("Automatic switching paused for 15 minutes." in the log); Resume ended it. Win+Alt+H toggled HDR on and off. With `toggle_hdr = "ctrl+alt+f9"`, `cycle_mode = ""`, `pause = "nonsense"`: Win+Alt+H did nothing, Ctrl+Alt+F9 toggled HDR both ways, Win+Alt+P paused (default for invalid text, logged), Win+Alt+O left the mode alone. In the window: the boxes showed Ctrl+Alt+F9, empty and Win+Alt+P; typing `ctrl + win + m` saved `Win+Ctrl+M`; typing `Shift+H` kept the old keys and showed "Add Win, Ctrl or Alt, so typing doesn't set it off."; Win+Ctrl+M was refused by Windows (another program holds it) and the section said "Win+Ctrl+M is used by another program, so it doesn't work."
Not verified: Narrator reading the boxes; the shortcut column of the tray menu by eye (UI Automation gives the item names without it).

Later the same day, captured from the screen with this branch's build as the tray: the menu shows Win+Alt+O in the shortcut column beside Automatic and an arrow on Pause automatic switching; the quick panel opened with both displays, the mode and Pause 1 hour. (A posted click carries no right to the foreground, so the panel was started with `huedra flyout` for the capture.)
