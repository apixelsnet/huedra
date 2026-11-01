# Changelog

Changes that matter to people using Huedra (not the project's own build and workflow changes). Versions follow `MAJOR.MINOR.PATCH`; the updater installs only versions newer than the running one.

## Unreleased

Nothing yet.

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
