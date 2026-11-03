# Changelog

Changes that matter to people using Huedra (not the project's own build and workflow changes). Versions follow `MAJOR.MINOR.PATCH`; the updater installs only versions newer than the running one.

## Unreleased

### Added
- The Huedra browser extension is on the Chrome Web Store. **Get the extension** on the **Video players** page, under Browsers, opens its store page, for Chrome, Edge, Brave, Opera and Vivaldi. The setup guide now installs it from the store; Firefox still loads it from the release's extension zip.

## 1.0.2 — 2026-11-02

### Fixed
- Closing Huedra closes all of it. When the tray process ends for any reason (Exit in the tray menu, `huedra exit`, an update, or a crash), the Huedra window closes too instead of staying open with "Huedra is not running". A window opened while Huedra was not running still stays, so **Start Huedra** can be pressed.
- Only one Huedra window opens. Clicking the tray icon again while the window was still opening (right after signing in, for example) could open a second one, and Exit closed only one of them.
- Exit no longer closes a File Explorer window that is open on a folder named Huedra, and Open Huedra no longer brings such a window to the front instead of opening Huedra's.

## 1.0.1 — 2026-11-02

### Fixed
- `huedra exit` and `huedra uninstall` close the Huedra window too, as Exit in the tray menu does. Before, a window that was open stayed open with no Huedra running behind it.

### Changed
- Support for the project moved from Ko-fi to Buy Me a Coffee: **Buy me a coffee** under General › About and **Support Huedra on Buy Me a Coffee...** in the tray menu open https://buymeacoffee.com/pixelsthecoder.

## 1.0.0 — 2026-11-02

The first stable release. Every feature was tested end to end before it.

### Changed
- Huedra is no longer a beta: the disclaimer and the README no longer call it beta software, and the release is not marked Pre-release.

### Fixed
- `huedra launch` with a program that cannot be started (a wrong path, for example) no longer turns HDR on and off again; it reports the problem and exits with code 2, and HDR stays as it was.
- Setting a display to not managed under Displays while Huedra has HDR on there returns it to SDR. Before, it stayed in HDR until it was turned off by hand.
- General › Start with Windows shows whether this copy of Huedra starts with Windows. A second or portable copy showed the other copy's entry as its own, and turning the switch off there removed it.
- Screen readers name the HDR setting and the two times on a game's page under Games.

## 0.1.4 — 2026-11-02

### Fixed
- Check for updates and Download and install work when Huedra was started from Explorer. Before, both stopped at once with "The check did not succeed" and wrote nothing to the log. To get this version on 0.1.1 to 0.1.3, choose Install automatically under General › Updates, or run `huedra update` in a terminal.

## 0.1.3 — 2026-11-02

### Changed
- An update replaces the program and leaves no copy of the old version: `huedra.previous.exe` is deleted when the new version starts, and `huedra update --rollback` is gone. Settings are kept, as before.

## 0.1.2 — 2026-11-02

### Fixed
- Check for updates says that it is checking, and then what it found, under the buttons in General › About.
- Exit in the tray menu closes the Huedra window too.

## 0.1.1 — 2026-11-01

### Added
- A disclaimer on first start. Huedra does not run, and never changes HDR, until it is accepted; it can be read again under General › About.
- Check for updates and Install buttons under General › About.
- Links to star the project on GitHub and to support it on Ko-fi, under General › About. The tray menu has **Support Huedra on Ko-fi...** too.
- A user guide: `docs/USER-GUIDE.md`.
- The download includes `THIRD-PARTY-LICENSES.md`, the licences of the libraries built into Huedra.
- General › Closing this window: keep Huedra running in the notification area (the default) or quit Huedra when the window is closed.
- Problem reports: after a crash, the next start offers to copy the details (versions, the crash and the last log lines, without your user name) to the clipboard. **Report a problem...** in the tray menu does the same at any time. Huedra sends nothing itself.
- The Status page shows a colour chart of the BT.2020 and sRGB gamuts that fades into colour when HDR turns on and back to grey when it turns off. The navigation pane shows the current state and its reason.
- An **Update available** dialog: when a new version is found it shows what is new, with a link to the full changelog, and installs it with **Download and install**. It never opens while a game or video is using HDR. The list of changes is signed like the update itself.
- Displays › Always HDR on the main display only, on by default: Always HDR mode and the Win+Alt+H shortcut turn HDR on only for the main display (the Windows primary display), so a second monitor stays in SDR. Turn it off to switch every managed display.

### Changed
- Starting Huedra opens its window, and starting it again while it runs brings the window to the front. At sign-in, Start with Windows still starts it in the notification area only (`huedra --background`).
- Updates now ask before installing by default, with the update dialog; Install automatically is still available under General › Updates.
- "Hold time" is now called "Keep HDR on after it ends", with a description of what it does, and the history says how long HDR stayed on.
- New icon: the visible colour gamut with the BT.2020 triangle, in colour while HDR is on and as a grey outline in SDR. Used for the program, the tray, the settings window and the browser extension. The settings window draws it as a vector image, sharp at any display scale, and shows it in the title bar.

## 0.1.0 — 2026-11-01

First release.

### HDR switching
- Turns HDR on for games and HDR video and back off afterwards, per display: only the display showing the content switches.
- Tray with Automatic, Always HDR and Always SDR modes, pause, keyboard shortcuts and a per-user command channel; the tray menu opens at once.
- `huedra` command line: switch HDR on a display, status, launch, update, uninstall. Retries and a recovery record if a switch fails part way.

### Games
- Reads the game libraries of Steam, Epic, GOG, Xbox, Ubisoft Connect, EA app and Rockstar, with PCGamingWiki information and game icons.
- Launch with HDR: `huedra launch`, a Steam launch option, desktop shortcuts and a "Start with HDR" menu entry.

### Video
- HDR video in local players (mpv, MPC-HC and others) by window title.
- Browser extension for YouTube (and an optional rule for streaming sites in full screen); see `extras/browser/CHANGELOG.md`.
- Aware of RTX Video HDR for SDR video in full screen.

### Settings and updates
- Settings window with Status, Games, Video players, Displays and General pages, usable with the keyboard alone; every control has a name for screen readers.
- Signed updates: a daily check (Install automatically, Ask before installing, or Off), `huedra update`, `--check`, `--from <folder>` and `--rollback`. A release is verified with minisign and SHA-256 before anything is replaced, and the previous version is kept.
- Starts with Windows (set up on first start); `huedra uninstall` removes every registry entry Huedra wrote.
- Timing lines in the log: start-up, game library scan, settings window and switch requests.

### About this release
- Beta: releases before 1.0.0 are pre-releases for testing.
- Licensed under the Apache License 2.0.
