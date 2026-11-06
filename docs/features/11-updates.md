# 11 — Updates

## Scope
- Default "Install automatically"; also "Ask before installing" and "Off" (Settings, General page).
- Check once a day; install only while no game or video is running; keep the previous version for a one-step rollback.
- Verify the minisign Ed25519 signature against the key built into the exe, then SHA-256, then replace by rename.
- WinHTTP; no TLS crate.

## Notes
- Hosting is deferred. The release address and the public key are set when the program is built (`HUEDRA_UPDATE_URL`, `HUEDRA_UPDATE_KEY`, passed through by `build/remote-build.sh` from `build/local.env`). Without both, the build never checks and the General page says so. Once hosting is chosen, both values are public and can be committed (for example in a `build/release.env`).
- A release is three files at the address: `release.toml` (version, file name, size, SHA-256 of the program), `release.toml.minisig`, and `huedra.exe`. Only the small manifest is signed; the program is trusted through the signed size and digest. On GitHub the address is the fixed release `update-channel` (`https://github.com/<owner>/<repo>/releases/download/update-channel/`), which every release refreshes; GitHub's `releases/latest/download/` would skip the beta pre-releases.
- Signing key: made once with `minisign -G` (password-protected), kept on the owner's PC outside the repository. `build/pack-release.ps1 -SecretKey <path>` signs. minisign for Windows: `winget install jedisct1.minisign`. Losing the key means a new key in a new build, and users must update by hand once.

## Done when
A signed test release installs, a tampered one is refused with a clear message, and rollback restores the previous exe.

## Result (2026-11-01) — phase 5
Code: `crates/huedra-core/src/update.rs` (manifest, signature check, version order, `update.toml`), `crates/huedra/src/update.rs` (`huedra update`), `crates/huedra-win/src/hash.rs` (SHA-256 through Windows CNG), `pipe::wait_closed`, tray schedule and menu in `crates/huedra/src/app.rs`, `build/pack-release.ps1`.
- Order of trust: the signature is checked before the manifest is read; the program must match the signed size and SHA-256, and `huedra.new.exe --version` must print the signed version before anything is replaced. Only newer versions are offered, so an old signed manifest cannot downgrade. A version rolled back from is recorded (`skipped`) and not installed automatically again; `huedra update` still installs it.
- Install: write `huedra.new.exe` next to the program; remove the old `huedra.previous.exe`; close the tray (pipe `exit`, then wait until the pipe is gone); rename `huedra.exe` to `huedra.previous.exe` and the new file into place (a running program can be renamed); start the tray again. A failed second rename puts the old program back. A folder that cannot be written is reported, with the advice to replace the file by hand.
- Tray: a timer for the next check (at least 2 minutes after start, then every 24 h) exists only in builds that can update and when Updates is not Off. When due and nothing uses HDR (no active trigger, no HDR held by Huedra), it starts `huedra update --auto` as its own process, so WinHTTP never loads into the tray. "Ask before installing" records the version and tells the tray (`update-available` request), which adds "Install Huedra x.y.z" to its menu. The status reply carries `busy`, which the updater checks again before installing.
- `huedra update --check`, `--from <folder>` (release files from a folder, still signature-checked) and `--rollback`. Every result goes to the log, because a run started from the tray menu has no console.
- SHA-256 uses Windows CNG instead of the `sha2` crate the plan named. `minisign-verify` 0.2.5 (MIT, no dependencies). The update code is linked only when a key is built in: 9,883,136 bytes without, about 9,940,000 with (about 60 KB under the 10 MB limit).

Verified: unit tests with key pairs made by minisign 0.11 (signed manifest read; changed manifest, other key, damaged signature, damaged key refused; size and digest mismatch; version order; daily due time; record round trip; release address rules; command forms). On this PC, with a throwaway key built into two builds (0.1.0 and 0.1.1) and a portable copy in Always SDR mode:
- `update --check --from` reported 0.1.1 and recorded it as available.
- A manifest changed after signing: "the release is not signed with Huedra's key (The signature verification failed)"; one flipped byte in the program: "the downloaded program does not match its signed SHA-256 digest". Nothing was replaced; both reasons were logged.
- `update --from` closed the running tray, swapped the files (`huedra.exe` 0.1.1, `huedra.previous.exe` 0.1.0) and started the tray again (new process, answering `status`). Running it again: "Huedra 0.1.1 is up to date."
- `update --rollback` swapped back (0.1.0 running, 0.1.1 kept as previous), restarted the tray, and recorded `skipped = "0.1.1"`.

Not verified: downloading over HTTPS from a real release address (no hosting yet; the WinHTTP path is the one PCGamingWiki lookups use); the daily `--auto` run and its "Ask" menu item, which need that address; the busy refusal during a game; a folder without write access; signing with a password-protected key and `pack-release.ps1 -SecretKey` (minisign is not installed on this PC; the test signatures were made in the build container).

Status: built; network half waits for hosting.

## Result (2026-11-01) — update channel and Check for updates
- The release address of GitHub builds is the fixed pre-release `update-channel` (`releases/download/update-channel/`), refreshed by every release, since `releases/latest/download/` skips the beta pre-releases.
- `update::project_page()` derives the repository page from that address for the Star on GitHub button; `update::SUPPORT_URL` is the Ko-fi page.
- Settings › General › About: Check for updates and Install (see 08). The note now says to choose Install under About or in the tray menu. (Since the UI review: Settings › About, and a version waiting shows as an update notice on Status and Settings with **See what's new**.)
Verified: the 0.1.0 beta release published `huedra.exe`, `release.toml` and its signature to `update-channel`. Not verified: an update from one release to the next through the channel (needs a second release and a public repository).

## Result (2026-11-01) — update dialog with signed release notes
Code: `ReleaseNotes`, `NoteLine`, `notes_path`, `changelog_page` in `crates/huedra-core/src/update.rs`; `keep_notes` in `crates/huedra/src/update.rs`; `offer_waiting_update` in `crates/huedra/src/app.rs`; `offer_update` and the dialog in `crates/huedra-ui/src/app.rs` and `ui/app.slint`; `.github/workflows/release.yml`.
- Updates: Ask before installing is now the default (a setting already written to the file keeps its value). When the daily check finds a newer release, the tray opens the settings window once no game or video uses HDR; the window shows the Update available dialog: the version, What's new (up to six changes with their headings, the list scrolls), "And N more changes", See the full changelog (CHANGELOG.md on GitHub), Later and Download and install. The dialog shows once per version per window; Check for updates opens it again; the tray menu entry "Huedra X is available..." opens it too.
- The notes are `release-notes.md` (first line `# Huedra <version>`, then the version's changelog section) with a minisign signature, on the update channel next to `release.toml`. They are a separate file because 0.1.0 refuses a manifest with fields it does not know. The check verifies the signature and the version line before keeping them as `update-notes.md` beside `update.toml`; without valid notes the dialog links to the changelog only.
- Download and install runs `huedra update`. When the program in place then reports a newer version, the window closes and opens again on the new version (the tray restarts itself); when there was nothing newer, it says so; on failure it gives the likely reason. `huedra update` now also clears a waiting version when the release address has nothing newer.
- The release workflow writes, signs and uploads the notes (before the manifest).

Verified: unit tests (signed notes read; another version, another key or changed bytes refused; summary keeps the headings of the changes shown; markdown links, bold and code marks removed). Build green, 101 core tests, exe 9,993,728 bytes. On this PC on a hidden desktop with a waiting 0.1.1 and notes from the Unreleased section: the dialog showed the version line, six changes, "And 4 more changes." and the three buttons; Download and install ran the updater, which could not read the release address and the dialog reported that the update was not installed.
Not verified: a real update through the dialog. The update channel answers 404 while the repository is private (GitHub serves release files of private repositories only to signed-in users), so checks fail until the repository is public. Also not verified: the tray opening the dialog after a daily check, See the full changelog (opens a browser).

## Result (2026-11-01) — wording for the hold time
- The setting is now "Keep HDR on after a game or video ends (seconds)", per game "Keep HDR on after the game closes", for video "Keep HDR on after the video ends", each with a description of why it exists. The history reason reads, for example, "Cyberpunk 2077 closed; HDR stayed on for 10 more seconds". The settings file keeps `hold_secs`.

## Result (2026-11-01) — first update through the channel
Verified on this PC with the published releases, the repository now public: a portable copy of the downloaded 0.1.0 zip ran `huedra update --check` against the real release address ("Huedra 0.1.1 is available"), then `huedra update` installed 0.1.1 (its SHA-256 matches the signed `release.toml`) and kept 0.1.0 as `huedra.previous.exe`; `update --rollback` restored 0.1.0, `update` installed 0.1.1 again, and 0.1.1 then reported itself up to date. The update channel answers 200 for every file.
Not verified: the daily check and the Update available dialog in a running tray (the test copy ran no tray); Install automatically.

## Result (2026-11-02) — no rollback; settings kept
Code: `remove_previous` and `install` in `crates/huedra/src/update.rs`; `run` in `crates/huedra/src/app.rs`; `verdict`, `UpdateState` in `crates/huedra-core/src/update.rs`; `parse_update` in `crates/huedra-core/src/command.rs`.
- Owner's decision: an update leaves only the new program. Windows cannot delete a running program, so `install` still moves the old one aside as `huedra.previous.exe`; the tray of the new version deletes it on start, in the background, trying every 0.5 s for 30 s while the process that ran the update lets go of it, and again on the next start otherwise.
- `huedra update --rollback`, `Verdict::Skipped` and `skipped` in `update.toml` are gone; an `update.toml` with `skipped` still loads.
- Settings: an update writes only in the program folder. Settings (`%APPDATA%\Huedra`), the log, update record and game information (`%LOCALAPPDATA%\Huedra`), or a portable copy's `data` folder, are untouched. Checked on the owner's installed copy after 0.1.1 to 0.1.2: `config.toml` kept every choice (`on_close = "quit"`, `updates = "install"`).

Verified on this PC (scratch portable copy, Always SDR): with a `huedra.previous.exe` beside it, the tray started with `--background` had deleted it within 3 s; `huedra update --rollback` is refused (exit code 2). Build green (109 tests), Defender clean.
Not verified: deletion right after a real update (the updating process holds the file; covered by the retries).

## Result (2026-11-02) — Check for updates and Install from a window opened through Explorer
Code: `release_own_console` in `crates/huedra-win/src/console.rs`; `run_program` and `on_start_huedra` in `crates/huedra-ui/src/app.rs`.
- Fault in 0.1.1 to 0.1.3: when Huedra was started from Explorer, Check for updates and Download and install in the window failed at once ("The check did not succeed"), with nothing in the log. The tray opens the window with `CREATE_NO_WINDOW`, so the window gets a hidden console; it released that console but kept its standard handles, which then pointed nowhere. `std::process::Command` duplicates those handles for the program it starts, the duplication failed (`ERROR_INVALID_HANDLE`), and `huedra update` never ran. Started from a shell, the window had no such handles, which is why earlier tests passed.
- Fix: `release_own_console` clears the three standard handles after `FreeConsole`. `run_program` gives the program no standard handles and logs a program that could not be started. Start Huedra in the window starts it in the background, as Accept does.
- Users of 0.1.1 to 0.1.3 who start Huedra from Explorer get this version through Install automatically, or with `huedra update` in a terminal.

Verified on this PC, with a portable copy started through Explorer as a double-click does: 0.1.2 reproduced the fault (no log line, no `update.toml`); this build said "Checking for updates...", then "Huedra 0.1.3 is available.", and Download and install updated it to 0.1.3, restarted it and left only `huedra.exe` in the folder. Build green, Defender clean.
Not verified: Windows 10 (no detached console policy; the same path applies).

## Result (2026-11-02) — 0.1.4 published; regression check in CI
- 0.1.4 published by the Release workflow; zip, `release.toml` and release notes signatures verified, the channel's `huedra.exe` matches the signed SHA-256, Defender clean. The owner's 0.1.2 started from Explorer still failed in the window, as expected, and was updated with `huedra update` in a terminal; settings kept (`on_close = "quit"`, `updates = "install"`). Started again from Explorer, 0.1.4 checked for updates from the window ("up to date") and deleted `huedra.previous.exe`.
- `build/check-child-start.ps1`, run by CI's Windows job (every release): starts `huedra launch -- cmd.exe /c exit 7` with `CREATE_NO_WINDOW` from a portable copy in Always SDR mode and expects exit code 7. The same path as the window's update buttons: release the console, then start a program with inherited handles.

Verified on this PC: the check fails on the 0.1.3 channel exe (exit 1, the program never started) and passes on 0.1.4.
Passes on GitHub's Windows runner too (manual CI run on this branch).
