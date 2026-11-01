# 11 — Updates

## Scope
- Default "Install automatically"; also "Ask before installing" and "Off" (Settings, General page).
- Check once a day; install only while no game or video is running; keep the previous version for a one-step rollback.
- Verify the minisign Ed25519 signature against the key built into the exe, then SHA-256, then replace by rename.
- WinHTTP; no TLS crate.

## Notes
- Hosting is deferred. The release address and the public key are set when the program is built (`HUEDRA_UPDATE_URL`, `HUEDRA_UPDATE_KEY`, passed through by `build/remote-build.sh` from `build/local.env`). Without both, the build never checks and the General page says so. Once hosting is chosen, both values are public and can be committed (for example in a `build/release.env`).
- A release is three files at the address: `release.toml` (version, file name, size, SHA-256 of the program), `release.toml.minisig`, and `huedra.exe`. Only the small manifest is signed; the program is trusted through the signed size and digest. A GitHub release fits: `https://github.com/<owner>/<repo>/releases/latest/download/`.
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
