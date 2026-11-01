# Changelog

Changes that matter to people using Huedra (not the project's own build and workflow changes). Versions follow `MAJOR.MINOR.PATCH`; the updater installs only versions newer than the running one.

## Unreleased

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
