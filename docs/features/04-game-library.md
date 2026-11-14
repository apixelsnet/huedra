# 04 — Game library

Finds installed games so that HDR can follow them without configuration.

## Scope
- Steam, Epic, GOG, Xbox, Ubisoft Connect, EA app, Rockstar, Battle.net, Riot Games, HoYoPlay, Amazon Games, itch.io, Heroic Games Launcher.
- The Xbox Game Bar's list of games, for games that no launcher lists.
- Manual addition by executable. Per-game mode, start delay and hold time.

## Notes
- Parsers (VDF, ACF, Epic manifests) contain no Windows code and are tested on Linux with sample files. Locating the files is in `huedra-win`.

## Done when
Every source lists its installed games on this PC in under 200 ms.

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

## Result (2026-11-03) — more sources
Code: `crates/huedra-core/src/library/{registry,amazon,itch,heroic,gamebar,mod}.rs` (parsers, tested on Linux), `crates/huedra-win/src/{stores,sqlite}.rs` (locating keys and files, SQLite queries), `crates/huedra-ui/src/app.rs` (a filter per source), `crates/huedra/src/games.rs`.
- Battle.net: uninstall entries (HKLM, both views) whose uninstaller is Battle.net's with `--uid=<product>`; the id is the product. The Battle.net app, test realms (`… Test`, `PTR`) and betas are skipped.
- Riot Games: the current user's uninstall entries `Riot Game <product>.<patchline>`; only `live` (no PBE), not the Riot Client. League of Legends counts only from its `Game` folder, so its client, open between matches, does not turn HDR on.
- HoYoPlay: `HKCU\Software\Cognosphere\HYP\1_0\<game>` and `HKCU\Software\miHoYo\HYP\1_0\<game>` (`GameInstallPath`, empty when not installed); known codes get their titles (`hk4e` Genshin Impact, `hkrpg` Honkai: Star Rail, `nap` Zenless Zone Zero, `bh3` Honkai Impact 3rd), others the folder name.
- Amazon Games: `%LocalAppData%\Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite`, table `DbSet` (`Installed = 1`); the program from each folder's `fuel.json` (`Main` › `Command`).
- itch.io: `%AppData%\itch\db\butler.db`, `caves` joined with `games`, only `classification = game`; folder and program from the install's `verdict` (`basePath`, first `windows` candidate that is not a helper).
- SQLite is read through `winsqlite3.dll`, part of Windows 10 and 11, loaded from System32 only (`LOAD_LIBRARY_SEARCH_SYSTEM32`) and only when the file exists; opened read-only with a 500 ms busy timeout. No dependency and no DLL shipped.
- Heroic: `%AppData%\heroic\legendaryConfig\legendary\installed.json` (Epic), `gog_store\installed.json` with each folder's `goggame-<id>.info` (title, primary play task), `nile_config\nile\installed.json` with `fuel.json` (Amazon; title from the folder). Add-ons and non-Windows builds are skipped. Ids `heroic:epic-…`, `heroic:gog-…`, `heroic:amazon-…`.
- Game Bar: `HKCU\System\GameConfigStore\Children\*` (`MatchedExeFullPath`), last. Named after the first folder above the program that is not generic (`Binaries`, `Win64`, `Versions\version-…`, `current`, `_retail_`…); that folder is the install folder. Left out: programs that no longer exist, programs inside a game already found, launchers and runtimes (`javaw`, `LeagueClientUx`, store apps), helpers, programs under the Windows folder or a `Temp` folder, and programs with no folder of their own. A Game Bar entry for a program added by hand is dropped (`library::without_repeats`).
- Xbox app games were already read from each drive's `.GamingRoot`, which covers custom install folders.
- Epic: the manifest folder is now taken from the launcher's `AppDataPath` (`HKLM\SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher`) when it exists, otherwise the default under `%ProgramData%`.

Verified on this PC: `huedra games` (build of this change) lists 30 games in 93 ms: the 22 from before, VALORANT, League of Legends (its `Game` folder) and Teamfight Tactics from Riot (the League PBE and the Riot Client left out), Genshin Impact and Zenless Zone Zero from HoYoPlay (Honkai: Star Rail, listed but not installed, left out), and Roblox, osu!lazer (`osulazer`) and mGBA from the Game Bar. Steam, Riot and Xbox games that the Game Bar also lists were not repeated. The Battle.net app's own entry was left out (no Battle.net games installed). Defender found no threats in the exe.
Epic: with Undying installed through the Epic Games Launcher, `huedra games` lists it as an Epic game (`C:\Program Files\Epic Games\Undying`, program `Undying.exe` from its manifest); 31 games in 14 ms.
Verified by the owner on this PC (2026-11-03): tested the build of this change and reported that it works.
Not verified: Battle.net games, Amazon Games, itch.io and Heroic (not installed here; covered by parser tests with sample data in the documented formats); the SQLite reader against a real database.

## Result (2026-11-07) — games added by hand match only their program
Code: `Library::find` and `is_windows_folder` in `crates/huedra-core/src/library/mod.rs`; tests `finds_by_folder_and_exe`, `windows_programs_never_match_by_folder`.
- Owner's report: Character Map (a test game added by hand from `C:\Windows\System32`) showed as running and turned HDR on when Windows Settings was opened, though Character Map never ran. A game added by hand counted every program in its folder, so a Windows helper started by Settings from System32 was taken for it. The same would happen for a game added from Downloads or the desktop.
- Now a game added by hand matches only its own program. Store games still match any program in their install folder (those folders are theirs alone), but nothing inside the Windows folder matches by folder. A launcher that starts another program is still followed by the tracker.

Verified: unit tests (the program itself still matches; another program beside it, a Windows helper and the Windows folder do not); CI green.
Not verified: on this PC with Windows Settings opened against the test copy's Character Map entry.

## Result (2026-11-14) — Auto HDR per game
Code: `crates/huedra-core/src/autohdr.rs`, `gpu_preferences` and `set_gpu_preferences` in `crates/huedra-win/src/registry.rs`, `note_program`, `sync_auto_hdr` and `set_auto_hdr` in `crates/huedra/src/app.rs`, the Auto HDR dropdown on a game's page in `crates/huedra-ui/ui/pages.slint`.
- A game's page has **Auto HDR**: As set in Windows (default), On, Off (`auto_hdr` in the game's `[[game]]` entry). Windows keeps the per-program choice in `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`, a string value named after the program's full path holding `Name=value;` pairs. The values were taken from Windows Settings › System › Display › Graphics on this PC (build 26300) by turning a program's Auto HDR switch on and off: `AutoHDREnable=4147` for on, `AutoHDREnable=48` for off. The global switch writes `AutoHDREnable=1` or `0` into `DirectXUserGlobalSettings`, which Huedra never touches. The per-program switch shows in Windows Settings only while HDR is on.
- Huedra changes only the `AutoHDREnable` pair and keeps the program's other pairs (`AppStatus`, `SwapEffectUpgradeEnable`…) in place; a value with nothing else left is deleted.
- Which programs: a game added by hand gets it at once on its program. Store games name only a folder, so the tray sets it on each of the game's programs it sees running (it applies from the next start, since Windows reads it when the game starts) and remembers them in `programs`. Choosing As set in Windows takes Huedra's pair off those programs and forgets them; a value set in Windows Settings for a program Huedra never touched stays as it is.

Verified on this PC (portable test copy, a copy of Notepad added by hand as "Auto HDR test"): with `auto_hdr = "on"` the tray's start wrote `AutoHDREnable=4147;` and remembered the program; on the game's page Down chose Off (`AutoHDREnable=48;`), Up twice chose As set in Windows (value deleted, `programs` emptied); the log named the game and the program. Unit tests cover keeping the other pairs.
Not verified: a store game's program being picked up while it runs (the same `set_auto_hdr`, reached from `note_program`); that Windows then applies Auto HDR in a DirectX 11 or 12 game (no SDR game was started).

