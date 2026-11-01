# Changelog

All notable changes. Versions follow `MAJOR.MINOR.PATCH`; the updater installs only versions newer than the running one.

## Unreleased

### Added
- Signed updates: a daily check (Install automatically, Ask before installing, or Off), `huedra update`, `huedra update --check`, `--from <folder>` and `--rollback`. Releases are verified with minisign and SHA-256 before anything is replaced; the previous version is kept.
- Start with Windows is set up on first start; a start-up entry pointing at a moved program is corrected.
- `build/pack-release.ps1` packs the release zip, `release.toml` and signatures.
- Timing lines in the log: start-up, game library scan, settings window, and switch requests.
- README, SECURITY.md and this changelog; the browser extension has its own in `extras/browser/CHANGELOG.md`.
- GitHub Actions: every push builds and tests in the same container as the local build, runs `huedra.exe` on Windows, and packs the release zip and the extension zip. A `v<version>` tag publishes a GitHub release with this file's section as its notes, signed when the signing key is set as a secret.

### Fixed
- The tray menu, a click on the tray icon and display changes no longer wait for an unrelated event: Explorer sends these messages directly, and the tray now wakes itself for them.

## 0.1.0 — development builds

- HDR switching per display from the command line, with retries and a recovery record.
- Tray process with automatic, Always HDR and Always SDR modes, pause, keyboard shortcuts and a per-user command channel.
- Game libraries of Steam, Epic, GOG, Xbox, Ubisoft Connect, EA app and Rockstar; PCGamingWiki information and game icons.
- Settings window with Status, Games, Video players, Displays and General pages.
- HDR video in local players, the browser extension for YouTube, optional streaming-site rule, RTX Video HDR awareness.
- Launch with HDR (`huedra launch`, Steam launch option, desktop shortcuts, "Start with HDR" menu entry) and `huedra uninstall`.
- HDR follows the display showing the content.
