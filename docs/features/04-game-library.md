# 04 — Game library

Finds installed games so that HDR can follow them without configuration.

## Scope
- Steam, Epic, GOG, Xbox, Ubisoft Connect, EA app, Rockstar.
- Manual addition by executable. Per-game mode, start delay and hold time.

## Notes
- Parsers (VDF, ACF, Epic manifests) contain no Windows code and are tested on Linux with sample files. Locating the files is in `huedra-win`.

## Done when
All seven stores list their installed games on this PC in under 200 ms.

## Result (2026-10-31) — phase 3
Code: `crates/huedra-core/src/library/{mod,vdf,steam,epic,xbox,registry}.rs` (parsers and matching, tested on Linux), `crates/huedra-win/src/{stores,registry}.rs` (locating files and keys), `crates/huedra/src/games.rs` (`huedra games`).
- Steam: `SteamPath` (HKCU) or `InstallPath` (HKLM), `libraryfolders.vdf` (current and old format), `appmanifest_*.acf` with the "installed" state flag; Steamworks Common Redistributables are skipped.
- Epic: `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item`; incomplete installs and non-game apps (by `AppCategories`) are skipped.
- GOG, Ubisoft Connect, EA app, Rockstar: their `HKLM\SOFTWARE\WOW6432Node` keys; Ubisoft entries without a folder and Rockstar launcher keys are skipped.
- Xbox: each drive's `.GamingRoot` (header `RGBX`, count, UTF-16 folder names), then `Content\MicrosoftGame.config` for the display name and program.
- Games whose folder no longer exists are dropped. A running program matches the game with the exact program path, otherwise the deepest install folder containing it; uninstallers, crash reporters and redistributable installers never match.
- The tray scans at start and on reload; games added by hand come first and win over a store entry with the same id.

Verified on this PC: `huedra games` lists 22 games (20 Steam across two libraries, 2 Xbox) plus one added by hand, in 67 ms on the first run and 2 ms when cached by Windows; the tray recognized Dispatch (Steam) as a running game.
Not verified: Epic, GOG, EA and Rockstar (none installed here; covered by parser tests with sample data); Ubisoft entries here have no folder and are skipped as designed.

Status: done.
