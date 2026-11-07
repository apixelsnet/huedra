# 08 — Settings window

The Slint window, run as its own process (`huedra settings`).

## Scope
- Pages: Status, Games, Video players, Displays, and Settings pinned at the foot of the navigation (General before 2026-11-07).
- Resizable: default 1040 x 680, minimum 520 x 560. Layouts: wide (1000 px and up), medium (680 to 999), compact (below 680, icon rail).
- Size, position and maximized state remembered. Per-monitor DPI awareness v2; follows the Windows text size.
- Palette from the BT.2020 primaries; spectrum only while HDR is on, gray in SDR.
- Motion: HDR fade 400 ms, page change 150 ms, controls 120 ms, hover 80 ms; none when Windows "Animation effects" is off.
- Rendering: FemtoVG (OpenGL) with the software renderer as fallback. Slint royalty-free license: attribution line on the About page.

## Done when
Every page works by keyboard, in light and dark, in all three layouts, at 100%, 150% and 200% scaling, and the window opens in under 300 ms.

## Result (2026-10-31) — phase 3
Code: `crates/huedra-ui/ui/{theme,pages,app}.slint`, `crates/huedra-ui/src/{app,lookups}.rs`, `crates/huedra-ui/build.rs`, `crates/huedra-core/src/geometry.rs`, `crates/huedra-win/src/{instance,shell}.rs`.
- Slint 1.18.1, Fluent style (follows Windows light or dark), winit backend, software renderer chosen in code. Compared on this PC, OpenGL (FemtoVG) used 91 MB of private memory and the software renderer 8.6 MB, so FemtoVG is not built at all. The spectrum is a 640 x 160 image drawn in Rust (`icon::spectrum_strip`) because the software renderer showed seams in multi-stop gradients.
- Pages: Status (spectrum card that fades in over 400 ms while HDR is on, mode selector, pause, recent changes), Games (search, store filters, list, detail with HDR setting, delay, hold, PCGamingWiki information and suggestion, Add game, Scan again), Displays (managed switch per HDR display, peak brightness, retry attempts), General (start with Windows, updates, PCGamingWiki, default times, shortcuts, window titles in the log, About with the Slint attribution).
- Layouts: navigation 240 px wide (1000 px and up), 200 px (680 to 999), icon rail of 64 px below 680; on narrow layouts the game detail replaces the list with a "Back to the list" button. Pages cross-fade and rise 8 px in 150 ms; every duration is zero when Windows "Animation effects" is off.
- Every change is saved at once (atomic write) and the tray process is asked to reload. Start with Windows writes or removes `HKCU\...\Run\Huedra`; nothing else touches the registry.
- `huedra settings [status|games|video|displays|general]` opens on a page; a second request brings the open window to the front. Size, position and maximized state are kept in `window.toml` (local app-data); a position off every monitor is ignored.

Verified on this PC: all four pages render as designed in the dark theme at 100 % scaling in the wide layout; the Status page follows the tray within one second (a real game, Dispatch, showed "HDR is on" with both displays); 8.8 MB private memory per window; the window handle exists about 30 to 45 ms after the process starts.
Not verified: the light theme; the medium and compact layouts (an attempt was stopped because a game was in use on this PC); keyboard-only use; 150 % and 200 % scaling; the Add game dialog and the Start with Windows switch. The controls take the Windows accent color (Fluent style) rather than ultramarine.

## Result (2026-10-31) — open items
Code: `crates/huedra-ui/ui/{theme,pages,app}.slint`, `crates/huedra-ui/src/app.rs`.
- Keyboard: Tab reaches every control, hidden pages are skipped, and a white (dark) or ink (light) outline shows keyboard focus only. Up and Down change the page in the navigation pane; Left and Right change the mode; Space or Enter picks a store filter; in the game list Up, Down, Home and End move and Enter or Space opens the game, scrolling the row into view. The mode is shown at once instead of after the next status poll, so quick arrow presses step through.
- The detail panel of the open game is updated in place, so a control keeps the focus after each change; it is rebuilt only for another game, a reset of the times or an applied suggestion. The store filters are replaced only when their counts change. Before, each change rebuilt both and the focus was lost.
- Switches and the "Start Huedra" button are drawn in the ultramarine accent (`Toggle`, `ActionButton`) instead of the Windows accent color.
- Fixed rows that cut off wrapped text: a page taller than the window was laid out at its minimum height (the scroll views now take the preferred height), and a row is never taller than the smallest maximum of its parts, so fixed-height controls are centered between stretchable spacers (`Centered`).
- With a game open in the wide layout the list hid the game names; the store and setting columns now give way while the detail panel is open.
- `compact` and `wide` are set when the width changes instead of bound to it, which removes the binding-loop warnings Slint gave for the layout; the unused `Glyph` component is gone.

Verified on this PC: the light theme on all four pages (Windows switched to light for the test and back); the medium (850 px) and compact (600 px) layouts on all pages, with and without an open game; keyboard-only use of the navigation, mode, pause, search, filters, list, detail panel and switches; 150 % and 200 % scaling with `SLINT_SCALE_FACTOR` (sharp text, layouts switch at the scaled widths); Add game (file dialog, game added and opened, counts updated) and Remove this game; Start with Windows writes `HKCU\...\Run\Huedra` with the quoted program path and removes it again.
Not verified: real 150 % and 200 % Windows display scaling (simulated with `SLINT_SCALE_FACTOR`); a screen reader. Both are on the pre-release checklist (12). The remaining Windows-accent widgets and the oversized-window panic are resolved in the next section.


## Result (2026-10-31) — consistency and end-to-end pass
Code: `crates/huedra-ui/ui/controls.slint` (new), `crates/huedra-ui/ui/{pages,app}.slint`, `crates/huedra-ui/src/app.rs`, `crates/huedra-win/src/{instance,registry}.rs`.
- Every control is now Huedra's own: `ActionButton` (plain, or primary in the accent), `Toggle`, `SearchBox`, `NumberBox` (typed, arrow buttons, Up and Down, mouse wheel; clamped; applied on Enter or leaving the field) and `Dropdown` (Up and Down change it directly; Space or Enter opens the list, which takes Up, Down, Enter and Escape). The Fluent button, text box, spin box and combo box are no longer used, so no control takes the Windows accent color, which can be pink, the color of HDR labels here. One focus outline, one field underline (ultramarine while typing), one hover and pressed tint throughout. The executable is 9.29 MB, slightly smaller than with the Fluent widgets.
- The window follows the Windows light or dark setting through its own registry watch (`registry::watch_light_theme`, `RegNotifyChangeKeyValue`, no polling) and sets the color scheme itself. Slint's own tracking applied a change late and missed the next one while the window was open.
- The window's size is limited to 16 384 physical pixels in `WM_WINDOWPOSCHANGING` (`instance::limit_window_size`, a subclass of the window), which also catches resizes made by other programs; Slint's `max-width` did not.
- Every change made in the window is a read-modify-write of `config.toml` (`edit`), so it no longer undoes a mode set from the tray menu, a shortcut or the command line while the window was open. Before, saving any setting wrote back the mode the window had loaded, and the tray then switched to it.
- PCGamingWiki's ray tracing field is shown in words ("Supported", "Limited support", "Not supported", "Through a modification") and left out when unknown, instead of the raw "true" or "false".

Verified on this PC: all pages in dark and light, wide and compact; light to dark and back while the window is open (each within a second); the number box by arrow button, typing, Enter, Tab, the Down key and clamping to its maximum; the drop-down by mouse, by Up and Down, and through its list with Enter and Escape; search, list, detail panel, applied suggestion, reset of the times; keyboard only from the search field to an opened game; a window resized by a program to 850 x 65 000 and 40 000 x 900 pixels stayed open at 16 384 pixels; with the window open, `huedra mode sdr` followed by a change in the window kept Always SDR.

## Result (2026-10-31) — game icons
- The game list shows each game's icon at 32 px on the left and the detail panel at 40 px beside the title (`GameIcon`); until an icon arrives, or when there is none, the game's first letter on a tinted square. Icons arrive one by one while the list is already usable. Sources and caching: see 13.

## Result (2026-10-31) — Start with HDR
- Games detail: a "Start with HDR" section with the Steam launch option (read-only field and a Copy button that uses the field's own copy) for Steam games, or "Create a desktop shortcut" for games of other stores with a known program. General: toggle for "Start with HDR" in the menu of programs. See 06.

Not verified: the window was not opened for these parts (built and type-checked only).

Status: done.

## Result (2026-10-31) — Video players page
- New page between Games and Displays (`huedra settings video`); Displays and General move down one. See 05 for its contents. New control `TextBox`: a line of text applied with Enter or on leaving the field.
- Controls in setting rows on the Video players, Displays and General pages now carry the row's title as their accessible name; before, screen readers announced an unnamed switch, spin box or combo box.
- General's default hold time says it also applies to video.
- Test method that leaves the owner's screen alone: the window runs on a separate desktop (`CreateDesktop`, process started with `lpDesktop`), and a helper on that desktop captures it with `PrintWindow` and drives it through UI Automation and posted key messages.

Verified on this PC that way: the Video players page (see 05); the Games detail of a Steam game shows the Start with HDR section with the launch option holding the program's path and a Copy button; the General switch for the Explorer menu wrote `HKCU\Software\Classes\exefile\shell\Huedra.StartWithHdr` (text, icon, `launch -- "%1"`) and removed it again, leaving no empty `exefile` key.
Not verified: the Copy button (it would overwrite the owner's clipboard) and the desktop shortcut button (no game here from a store other than Steam or Xbox).

## Result (2026-10-31) — Displays: only the display showing the content
Code: `DisplaysPage` in `crates/huedra-ui/ui/pages.slint`, `show_displays` and `wire_displays` in `crates/huedra-ui/src/app.rs`.
- A switch above the display list, on by default: "Only the display showing the content".

Verified on this PC on a hidden desktop: the switch is on with no `content_only` line in the file; off wrote `content_only = false`, on again wrote `content_only = true`. Exe 9,859,072 bytes (about 141 KB under the limit).

## Result (2026-11-01) — phase 5
- General › Updates now says whether this build checks ("This build has no release address, so no checks are made."), when it last checked, and which version waits for consent (`update_note`, from `update.toml`).
- The log records "Settings window opened in N ms", from process creation to the first turn of the window's event loop (the window is shown by then; its first frame is drawn on that turn or the next).

Verified on this PC on a hidden desktop: the Updates text in a build without a release address (captured); opening times 51, 39, 38, 40 ms (target 300 ms).

## Result (2026-11-01) — names for screen readers
Code: `SearchField` and `Toggle` (`spoken-text`) in `crates/huedra-ui/ui/controls.slint`; `ModeSelector`, the game list and the display switches in `pages.slint`.
- Checked by reading the UI Automation tree of all five pages (what Narrator and NVDA read), on a hidden desktop. Before: the search field showed as two edit fields, one without a name; the mode selector and the game list had no name; both display switches read "Managed".
- Now: one edit field "Search 22 games"; "Mode" (Automatic, Always HDR, Always SDR); list "Games"; "Manage Display 1 · MO27Q3" and "Manage Display 2 · LG ULTRAGEAR" (the visible text stays Managed / Not managed). No interactive control is left without a name: 16 buttons and switches, 4 drop-downs, 4 number fields, 2 edit fields, 9 list items, 28 tabs.

Verified: CI green on the branch (build of the fix); the UI Automation tree above. Not verified: Narrator or NVDA actually speaking the pages (needs sound and a person); which page is current: the navigation items are tabs with a selected state, but UI Automation shows no selection pattern for them. They now take a screen reader's default action (Invoke), so a page can be opened from the screen reader as well as with Up and Down (checked in the UI Automation tree: all five navigation items and the three mode items have Invoke).

## Result (2026-11-01) — disclaimer, About actions, logo
Code: the disclaimer overlay and About properties in `crates/huedra-ui/ui/app.slint`, the About section in `pages.slint`, `wire_about` in `crates/huedra-ui/src/app.rs`, the tray's check in `run` in `crates/huedra/src/app.rs`, `general.disclaimer_accepted` in `config.rs`.
- First start: the tray does not start until the disclaimer is accepted (it opens the settings window instead and ends). The window shows "Before you start" over every page: what Huedra changes, beta and no warranty, no liability, anti-cheat, how to undo a stuck display. **Accept and continue** records `disclaimer_accepted = true`, adds the start-up entry as the tray's first start used to (non-portable copies with Start with Windows on), and starts the tray if none runs. **Exit** closes the window. **General › About › Disclaimer** shows it again with only **Close**.
- About: **Check for updates** (runs `huedra update --check` without a console, then shows the result), **Install Huedra x.y.z** when one is waiting (`huedra update`), **Star on GitHub** (the repository the release address belongs to; hidden for builds without a GitHub release address) and **Support on Ko-fi**.
- The window's logo is the gamut icon in both states (HDR color, SDR gray outline) instead of a dot and a ring.
Verified on this PC on a hidden desktop with the CI build: the disclaimer appears for a settings file without the flag, with Exit and Accept and continue; Accept wrote `disclaimer_accepted = true` and the Status page followed; About shows Check for updates, Disclaimer, Star on GitHub and Support on Ko-fi. Not verified: Check for updates and Install against a real newer release; the buttons opening the browser; the tray's start being held back on a fresh PC (the logic is in `run`, not exercised on this PC's running tray).

## Result (2026-11-01) — window icon, vector logo, color chart, navigation status
Code: `set_icon` in `crates/huedra-win/src/instance.rs`; `HeroCard` in `crates/huedra-ui/ui/pages.slint`; the navigation header in `crates/huedra-ui/ui/app.slint`; `crates/huedra-ui/ui/assets/`.
- Window icon: Slint did not pass its `icon` property on to the window (no small or large icon set, so Windows drew a generic one in the title bar). The window now gets the exe's icon resource with `WM_SETICON`, at the small and large icon sizes for its display scale; the unused Slint icon was removed.
- The settings window draws the icon and the Status card's color chart from SVG files embedded at build time, so they are sharp at any display scale (the raster versions were drawn at a fixed 44 px). SVG support was already in the exe; the exe grew by 56 KB to 9,894,400 bytes.
- Status card, as in the plan's mockup: a color chart of the BT.2020 triangle around the dashed sRGB triangle, gray in SDR with the sRGB triangle filled. While HDR is on, the spectrum fades into the chart, the card and its border over 400 ms, and back to gray in SDR (no motion when Windows animation effects are off). The headline turns pink in HDR.
- Navigation pane: the icon fades between gray and color, with the state ("HDR on" or "SDR") and one line below it, for example "Display 1 · Always HDR mode" or "No HDR content".
- Displays: a switch "Always HDR on the main display only", on by default (see 02).

Verified on this PC on a hidden desktop: the title bar shows the gamut icon (WM_GETICON returns small and large icons); the switch shows on with no `manual` line in the file, off wrote `manual = "all"`, on again `manual = "main"`; with the tray switching Display 1 to HDR, the card, chart and navigation status went from SDR to HDR and back (captured frames for the README tour).
Not verified: display scales other than this PC's, and the light theme with HDR on.

## Result (2026-11-01) — supporter note
- About: the support caption adds that supporters are thanked by name and get preview builds before each release. Every feature stays free; nothing is locked behind support (owner's decision).

Verified by the owner on this PC (2026-11-01, build a84f210): About shows the new caption.

## Result (2026-11-01) — Closing this window
Code: `on_close_requested` and `wire_general` in `crates/huedra-ui/src/app.rs`; General page in `crates/huedra-ui/ui/pages.slint`, `app.slint`.
- General › Closing this window: Keep Huedra running (default) or Quit Huedra. With Quit, closing the window sends `Exit` to the tray, which returns the displays it turned to HDR to SDR and closes. Exit in the tray menu always quits.
- The Dropdown adds about 49 KB to the exe (Slint expands each instance): 9,990,144 bytes; the limit was raised to 12 MB for it (see 12).

Verified on this PC: with `on_close = "quit"`, closing the window closed the tray as well ("Huedra closed." in the log); with the default, the tray kept running.
Verified by the owner on this PC (2026-11-01): the new row and choosing it in the Dropdown.

## Result (2026-11-01) — the update check says what it is doing
Code: `wire_about` in `crates/huedra-ui/src/app.rs`; `update-check-status` in `pages.slint`, `app.slint`.
- Check for updates showed only a brief "Checking..." on the button; the result went to the Updates row higher up the page. A line under the About buttons now says "Checking for updates...", then "Huedra <version> is up to date.", "Huedra <version> is available." (the update dialog opens as before) or "The check did not succeed; the log has the reason."

Verified on this PC through UI Automation, with a test build carrying the update key (it reports 0.1.0): during the check the button read "Checking..." and the line "Checking for updates..."; after it, "Huedra 0.1.1 is available." and the update dialog was open.
Not verified: the up-to-date and failed messages (they need a build of the newest version, and no network).

## Result (2026-11-02) — end-to-end pass of the window
Code: `starts_with_windows` in `crates/huedra-ui/src/app.rs`, `startup_starts` in `crates/huedra-core/src/command.rs`, labels in `pages.slint`.
- Start with Windows showed On whenever any Huedra start-up entry existed, so a portable or second copy showed the installed copy's entry as its own, and turning the switch off there deleted it. The switch now shows On only when the entry starts this program (path compared without case).
- The HDR setting box and the two seconds boxes on a game's page had no accessible name (UI Automation showed them as unnamed); they now carry their labels.

Verified on this PC, every page driven through UI Automation on a portable copy (the owner's settings and registry untouched, their start-up entry checked before and after):
- Disclaimer: shown first, no HDR change before Accept (even with Always HDR in the file), Accept writes the flag and applies the mode, Exit closes Huedra.
- Status: the three mode buttons switch HDR, Pause and Resume, the navigation status and the color-chart caption follow HDR, Recent changes lists the switches, a mode set with `huedra mode` shows in the window, starting Huedra again keeps one window.
- Games: library with store filters, search, Added filter, a game's HDR setting (Do not change then applied by the tray), delay and Use the default times, Create a desktop shortcut (target `huedra.exe launch -- "<program>"`, removed afterwards), the Steam launch option and Copy, PCGamingWiki results arriving, Scan again, Remove this game, Add game opening its file dialog (canceled).
- Video players: Only in full screen, keywords (Enter), a player set to While open (then applied by the tray), hold time, a streaming site.
- Displays: Only the display showing the content, Always HDR on the main display only (then both displays in Always HDR), Manage, Retry attempts.
- General: Start with Windows (Off in the portable copy), the Start with HDR menu entry written and removed, Updates, Game information, hold and delay, Keyboard shortcuts, window titles, the version, the disclaimer, Closing this window: Quit closes Huedra, Keep running leaves the tray.
- About › Check for updates in a window started through Explorer (build reporting 0.1.0): the Update available dialog names 0.1.4 and lists the changes; Download and install replaced the program, the window opened again on 0.1.4 and `huedra.previous.exe` was gone.
Not verified: the Copy button's clipboard text, the GitHub and Ko-fi buttons opening the browser, the light theme, other display scales, a screen reader speaking.

## Result (2026-11-02) — 1.0.0: not a beta
Code: the disclaimer text in `crates/huedra-ui/ui/app.slint`; `README.md`, `docs/USER-GUIDE.md`.
- The disclaimer no longer calls Huedra beta software ("Huedra is provided "as is", ..."); the README drops its beta note and the pre-release badge filter, and the guide points to the release marked Latest.

## Result (2026-11-02) — Buy Me a Coffee
Code: About in `crates/huedra-ui/ui/pages.slint`.
- About's support button reads **Buy me a coffee** and opens the same page as the tray item (`update::SUPPORT_URL`); the caption above it names Buy Me a Coffee. `.github/FUNDING.yml` uses `buy_me_a_coffee`, so the repository's Sponsor button links there too.

## Result (2026-11-02) — Closing closes everything
Code: `close_with_tray`, `start_status_polling` and the window claim in `crates/huedra-ui/src/app.rs`; `own_windows`, `close_window`, `claim_window` in `crates/huedra-win/src/instance.rs`.
- Once the window has seen the tray answer, it closes itself when the tray's pipe is gone (`PipeError::NotRunning`; a busy or failed call does not count), whatever ended the tray: Exit, `huedra exit`, the updater, a crash or Task Manager. The window running Download and install stays. A window opened while the tray was not running stays open for Start Huedra.
- One window only: the process showing it holds the named mutex `Local\Huedra-settings-window`; a second start waits up to 10 s for that window and brings it to the front. The claim is released before the window reopens after an update.
- Windows are found by title and by program name (`huedra.exe`, `huedra.*.exe`, any folder), not by title alone, so an Explorer window on a folder named Huedra is neither closed nor brought forward. Exit closes every match, not the first.

Verified: build green (fmt, clippy, tests, release), Defender scan clean. On this PC with the test build: three `huedra settings` started at once gave one window; `huedra exit` left no Huedra process; killing the tray process closed the window within 3 s; a window opened with no tray stayed, and closed on `huedra exit` after Start Huedra.

## Result (2026-11-07) — UI review, steps 1 and 2
Plan: the UI review artifact (owner's calls of 2026-11-05; open items decided on 2026-11-07: width cap and Settings pinned, yes).
Code: `crates/huedra-ui/ui/controls.slint` (`InfoBar`, `Expander`, `SubRow`, `FilterChip`, `IconButton`, `LinkButton`, `KeyCap`), `pages.slint` (`StatusPage` with `NowCard` and `DisplaysCard`, `GamesPage` with cue chips and the cover grid, `DisplaysPage`, `SettingsPage`), `app.slint` (navigation, wiring), `theme.slint` (`page-width`, `violet-text`); `crates/huedra-ui/src/app.rs` (`now_rows`, `cues`, `passes`, `show_displays`, failure notice, `tray`, `read_displays`, `scan_games`), `crates/huedra-ui/src/sample.rs`; `HistoryEntry.day` in `crates/huedra-core/src/ipc.rs`, `day_label` and `day_before` in `clock.rs`; `Page::Settings` (alias `general`).
- **Status** is a dashboard: the color chart card, then **Now running** (the game with its store and setting and a **Game settings** link that opens it on Games, or the video player with **Player settings**) beside **Displays** (every connected display with HDR/SDR badge and "Controlled by Huedra", "Not controlled by Huedra" or "No HDR support"; side by side from 760 px of page width, stacked below), the **Mode** card with **Pause** as its own row and the shortcuts as key labels (only while shortcuts are on), and **Recent changes** grouped by day (Today, Yesterday, then the date), five shown, **Show N earlier changes** for the rest. The window reads every display itself on its status thread each second, so displays Huedra does not control show their state too.
- **InfoBar**: one banner under the page title. Violet information ("Huedra is not running." with **Start Huedra**), pink with an outline for a failed switch (the newest change is a failure; closing it hides it until the next failure), ultramarine for a version waiting (**See what's new** opens the update dialog; on Status and Settings; closing it hides it until the window opens again). No yellow.
- **Games**: columns Game (store, or "Running · store" in pink), **HDR support** (badges HDR, Limited HDR, No HDR, HDR mod, Not recorded, Looking up; none for games added by hand or with game information off) and **Setting** ("Default" in gray unless the game has its own, then the setting in bold with "Own setting", or a violet "Suggests …" badge where PCGamingWiki points to another setting). Filters **Supports HDR**, **Own setting** and **Suggestions** after the store filters (on a second row below 1000 px). A notice "N games have a different suggestion." with **Review** (the Suggestions filter) and **Apply all N**. The columns give way as the page narrows or the detail panel opens (support from 560 px, setting from 820 px). **Cover grid** (toolbar buttons List / Cover grid): tiles of at least 128 px in a 2:3 shape, tinted by name, with the icon, the HDR badge and a violet dot for a suggestion; arrows move through it, Enter opens the game. Portrait art from the stores is not used (no third-party art; a later step can add Steam's local library art).
- **Displays**: the switch reads **Let Huedra control HDR** (spoken "Let Huedra control HDR on Display 1 · …"); its line says what happens ("Huedra turns HDR on and off here." / "Huedra does not change this display. HDR stays as you set it in Windows."); a display without HDR shows "Not available" instead of a disabled switch. The two switching settings follow under **How displays switch**.
- **Settings** (was General): Start with Windows, Closing this window and Updates stay rows; **Timing**, **Keyboard shortcuts** (switch in the header) and **Advanced** fold into expanders whose second line sums up the values and follows each change; **About** is an expander headed "Huedra x.y.z" with **Check for updates**, open by default (Support, settings folder, Disclaimer, PCGamingWiki and Slint attribution). Expanders open with a click, Space or Enter and expose UI Automation's ExpandCollapse pattern.
- **Frame**: pages stop at 1056 px (1000 px of content) and stay beside the navigation; **Settings** is pinned at the foot of the pane. Up and Down in the pane keep the page order.
- **Sample data** (test builds only): `bash build/remote-build.sh --sample` also builds `dist/huedra-sample.exe` (feature `sample`): twelve popular games with their PCGamingWiki HDR values, three displays named after popular monitors (ROG Strix XG27UCS, Alienware AW3423DWF, Dell P2422H without HDR), a history ending a few minutes before now, and a simulated tray: the mode, Pause and the display switches change the simulated HDR, nothing reaches a real tray, display, the registry or the network. `HUEDRA_SAMPLE_MODE=sdr|hdr`, `HUEDRA_SAMPLE_FAILURE=1`, `HUEDRA_SAMPLE_UPDATE=1.2.0` and `HUEDRA_SAMPLE_THEME=light|dark` set the starting state. CI runs clippy on it as well.
- `deny.toml`: RUSTSEC-2026-0441 (pin-utils unmaintained, through Slint's build) is ignored with a note; the ttf-parser notice no longer matches and was removed.

Verified on this PC with the sample build on a hidden desktop (a portable copy with its own settings; the owner's tray and displays untouched): every page in dark at 1100 px, light at 780 px and the compact rail at 600 px; Always SDR, Pause and Resume, Automatic; Display 2 switched off (`unmanaged` written, Status showed "Not controlled by Huedra") and on again; Timing and Advanced opened through UI Automation; the shortcuts switch wrote `hotkeys = false` and the header said "Off", then back; Apply all 4 wrote four `do-not-change` settings and the notice left; the failure and update notices; Show earlier changes and Show fewer; list, grid, the detail panel beside both, the Suggestions filter. UI Automation lists no unnamed control on Status, Games or Settings. Full CI build green (fmt, clippy for Linux, Windows and the sample feature, deny, 129 tests, docs); exe 10,965,504 bytes with the update key (limit 12,000,000); Defender scan clean for it and the sample build.
Not verified: the real tray (no game was started; the dashboard was fed by the sample), the cover grid with many games, a screen reader speaking, display scales other than this PC's.

## Result (2026-11-07) — Mica
Code: `run` in `crates/huedra-ui/src/app.rs` (`MICA_BUILD`), `mica` in `app.slint`, `registry::windows_build`.
- The test build asked Windows for the Mica backdrop (`BackdropType::MainWindow`, transparent window) with the navigation pane left without a background: the software renderer leaves those pixels transparent and Windows draws Mica behind them. Captured from the screen on this PC: the pane showed a blue tint varying from (27, 33, 42) to (30, 32, 36) against the page's (22, 20, 30). So it ships: on Windows 11 build 22621 and later the navigation pane shows Mica; earlier builds keep the solid color (a transparent pane would be black there). Pages, cards and controls stay solid, so the palette is unchanged where it matters. Sample builds use it only with `HUEDRA_SAMPLE_MICA`, since a capture of a hidden window cannot show it.
Not verified: the light theme with Mica, an inactive window (Windows shows Mica plainer then), "Transparency effects" off.

## Result (2026-11-07) — README images
- `docs/assets/huedra-tour.gif` and `docs/assets/screenshot-games.png` were taken again from the sample build (`dist/huedra-sample.exe`) on a hidden desktop: twelve popular games, the ROG Strix XG27UCS, Alienware AW3423DWF and Dell P2422H, simulated HDR, so no display of this PC switched and no network lookup was made. The tour: Status in SDR, Automatic turning HDR on for Cyberpunk 2077, Games as list and cover grid, Video players, Displays, Settings, the quick panel over the dimmed window, back to SDR (820 x 558, 12 fps, 279 frames, 4,528,459 bytes; the old tour was 2.7 MB with four scenes fewer). The Games screenshot is 1316 x 799, wide enough for the Setting column. `HUEDRA_SAMPLE_VERSION=1.2.0` names a release in About (the workspace says 0.1.0), and a sample build's Updates line reads "Last checked an hour ago." Every frame was checked: sample names only, the settings folder shown as `%LOCALAPPDATA%\Huedra`.

