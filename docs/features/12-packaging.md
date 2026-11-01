# 12 — Packaging and build

How Huedra is built, run, installed and removed.

## Scope
- Build on the server in Docker (`build/Dockerfile`, image `huedra-build`); cross-compile with `cargo-xwin` to `x86_64-pc-windows-msvc`.
- `build/remote-build.sh` syncs the source, runs `build/ci.sh` in the container and copies `huedra.exe` to `dist/`.
- `build/run.ps1` stops a running copy and starts the new one.
- `build/pack-release.ps1`: release folder with the zip, `release.toml` and minisign signatures (see 11).
- Start-up entry (HKCU Run) on first start; `huedra uninstall` removing every key Huedra wrote.

## Notes
- The server name is read from `HUEDRA_BUILD_HOST` or the git-ignored `build/local.env`; it never appears in committed files.
- `cargo-xwin` downloads the Microsoft CRT and SDK files. Their free terms are accepted in the image (`XWIN_ACCEPT_LICENSE=1`); no paid licence is involved.
- The C runtime is linked statically (`.cargo/config.toml`), so no Visual C++ redistributable is needed.
- Cache: Docker volumes `huedra-cargo`, `huedra-advisory`, `huedra-xwin` and the `build/huedra/target` folder on the server. Clear only these, and only before a release or measured build, after a toolchain bump, or when a build looks stale. Never a global Docker prune.
- Dependency checks use `cargo deny` (advisories, licences, bans, sources); a separate `cargo audit` would only repeat its advisory check.

## Done when
A clean clone builds with one command, and the release zip installs, starts with Windows and uninstalls without leftovers.

## Result (2026-10-31) — phase 0
Code: `build/Dockerfile`, `build/ci.sh`, `build/remote-build.sh`, `build/run.ps1`, `.cargo/config.toml`, workspace `Cargo.toml`.
- Image `huedra-build`: Rust 1.99.1 (bookworm), clang, lld, llvm, cargo-xwin 0.23.1, cargo-deny 0.20.2.
- `ci.sh` passes: fmt, clippy for Linux and for Windows with `-D warnings`, cargo deny (advisories, bans, licences, sources), tests (1), docs with `-D warnings`, Windows release build.
- `huedra.exe` is 214 016 bytes. It imports KERNEL32 but not `VCRUNTIME140.dll`, so the static C runtime works.
- `/IGNORE:4099` hides the linker's "no PDB for libcmt.lib" warnings; Microsoft does not ship those symbols.
- First run downloaded the Microsoft CRT and SDK files in 52 s; later builds reuse the `huedra-xwin` volume.

Verified on this PC: `build/run.ps1 --version` prints `Huedra 0.1.0` and exits 0; an unknown argument exits 2.
Not verified: a build from a fresh clone on another machine; the image build time from an empty Docker cache was not measured.

Open: cargo deny warns that allow-listed licences are unused, because there are no dependencies yet.

## Result (2026-10-31) — phases 1 and 2
- `build/remote-fmt.sh` runs `cargo fmt` on the server and copies the formatted crates back (Rust is not installed on this PC).
- `ci.sh` runs `cargo fetch` first, so new dependencies update `Cargo.lock` minimally and every later step runs `--locked`.
- `build.rs` embeds the manifest (`/MANIFESTINPUT`) and a `.res` with version information and the icon (`llvm-rc` from the image).
- Windows Defender quarantined the first phase 2 build, run from the Temp folder, as `Trojan:Win32/Bearfoos.A!ml` (a machine-learning heuristic) seconds after it switched HDR. After adding version information and the icon, and running from `dist\`, the same tests ran without detection. Code signing remains the real remedy and is still deferred.
- Executable size: 1.13 MB (the 256 px icon is about 260 KB of it).

## Result (2026-10-31) — phase 3
- `deny.toml` checks only the shipped target (`x86_64-pc-windows-msvc`); with both targets, Linux-only dependencies of Windows-only crates (zbus through Slint's winit backend) entered the graph. Newly allowed licences: BSD-2-Clause, BSL-1.0 and `LicenseRef-Slint-Royalty-free-2.0` (attribution shown in General > About). `RUSTSEC-2026-0192` (ttf-parser unmaintained, used by Slint) is ignored with a note.
- Release profile now `opt-level = "z"`: 10.2 MB with `"s"`, 9.3 MB with `"z"`. That is above the 8 MB target; most of the size is Slint's text and accessibility stack. Executable packers were ruled out because they trigger antivirus false positives.

## Result (2026-10-31) — open items
- Executable size after the settings-window fixes: 9.38 MB (9,375,744 bytes). Built without Slint's `accessibility` feature it is 9.19 MB, so the screen-reader support costs only 187 KB and stays. The rest is Slint itself (text layout and shaping, system fonts, number formatting data, SVG text), which its features do not switch off while `std` is on. The 8 MB target is not reachable without replacing the UI toolkit; it needs a new figure. After the settings window stopped using the Fluent widgets: 9.29 MB (9,292,288 bytes).

Size limit set by the owner: 10 MB (10,000,000 bytes). `build/ci.sh` fails the build above it. With game icons (PNG and JPEG decoding): 9.47 MB (9,472,512 bytes).

Pre-release checklist (testers on other machines, before the first public release):
- Windows 10, or Windows 11 before 24H2: HDR on and off through the older interface (01).
- A display at 150 % and at 200 % in Windows: the settings window and the tray icon (08, 07).
- Narrator or NVDA: every page of the settings window can be read and used (08).

## Result (2026-10-31) — phase 4
- `huedra uninstall` exists early because Launch with HDR writes a registry key: it closes the tray and removes every value and key Huedra writes (start-up value, "Start with HDR" menu entry). See 06.

## Result (2026-11-01) — phase 5
Code: `build/pack-release.ps1`, `startup_entry` in `crates/huedra/src/app.rs`, `uninstall` in `crates/huedra/src/launch.rs`, `minisign` in `build/Dockerfile`, update variables in `build/remote-build.sh`; `README.md`, `SECURITY.md`, `CHANGELOG.md`.
- `pack-release.ps1` reads the version from `huedra.exe --version` and writes `dist\release\<version>\`: `huedra.exe`, `release.toml` (LF, no byte-order mark) and `huedra-<version>-windows-x64.zip` (program, README, SECURITY, CHANGELOG, and LICENSE once there is one). With `-SecretKey` it signs the manifest and the zip with minisign.
- Start-up entry: written on the first start (no settings file yet) of a non-portable copy, because Start with Windows is on by default; rewritten when the entry names a program that no longer exists. Portable copies never touch it.
- `uninstall` now names both data folders (roaming and local).
- README follows Best-README-Template, with the questions from the plan (anti-cheat, what leaves the PC, SmartScreen, the black flash, removal, tested hardware).

Performance targets, measured on this PC (Windows 11 build 26300):

| Target | Result |
| --- | --- |
| Start-up to tray icon ≤ 100 ms | 20–46 ms from process creation (log line); 119 ms on the very first run of a newly copied exe (antivirus scan) |
| Library scan ≤ 200 ms | 3–4 ms; 91 ms on that first run |
| Settings window ≤ 300 ms | 38–51 ms to the first turn of its event loop (hidden desktop, 4 runs) |
| Tray memory ≤ 4 MB | 1.5 MB private working set, 2 threads, about 12 minutes after start |
| Idle wake-ups: none between events | not proven: about 995 context switches in 17 of 60 ten-second windows over 10 minutes, while other tests and the owner's own use produced foreground changes; a run with the PC untouched is still needed |
| Switch request ≤ 50 ms | not measured: the log line exists ("Switch requested N ms after the event") but no real switch was made in this session |
| Executable | 9,883,136 bytes without an update key, about 9.94 MB with one (limit 10,000,000) |

Verified on this PC: a non-portable copy's first start wrote `HKCU\...\Run\Huedra` with its quoted path and logged "Huedra will start with Windows."; `huedra uninstall` closed it and left neither the Run value nor the `exefile` menu key; no Huedra process remained. `pack-release.ps1` without a key packed a test release (zip 5.2 MB, manifest read back by the updater after signing in the build container).
Not verified: `pack-release.ps1 -SecretKey` with minisign on this PC; a clean idle measurement; tray memory after one hour; a release from a fresh clone.

Note: `uninstall` and `update` act on whichever tray owns the per-user pipe, including a copy started from another folder.

Status: done except the open measurements above; publishing waits for licence and hosting.

## Result (2026-11-01) — GitHub Actions
Code: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `build/check-changelog.ps1`, `extras/browser/CHANGELOG.md`.
- Same shape as the owner's Mango project: `ci.yml` checks pushes to `main` and pull requests and publishes nothing; `release.yml` publishes, started by a pushed tag `vX.Y.Z` or by Run workflow (patch, minor, major or an exact version), which tags the tip of `main` itself. The version comes from the tag and is written into `Cargo.toml` for that build only; the first release without tags takes the `Cargo.toml` version. `release.yml` calls `ci.yml` (`workflow_call`) with the version and tag, so a release runs every check.
- `build` (Ubuntu): builds `build/Dockerfile` (layers kept in the Actions cache) and runs `build/ci.sh` in it, as `remote-build.sh` does on the server. The Cargo registry, the Microsoft CRT and SDK files and `target` are kept in the Actions cache, keyed by the toolchain, the Dockerfile and `Cargo.lock`. Fails when `ci.sh` had to change `Cargo.lock`. Uploads `huedra.exe`.
- `windows` (Windows Server 2025): `huedra.exe --version` must print the expected version and an unknown option must exit 2; changelogs (`check-changelog.ps1`): `CHANGELOG.md` needs an Unreleased section, and for a release a section for the version or a non-empty Unreleased one; `extras/browser/CHANGELOG.md` needs a section for the extension's version. Then `pack-release.ps1` and `pack-extension.ps1`. Uploads the release folder and the extension zip (kept 7 days).
- `release.yml` `publish`: the program must be the tag's version; the notes are that version's section of `CHANGELOG.md`, else its Unreleased section, followed by GitHub's generated list; with the `MINISIGN_SECRET_KEY` and `MINISIGN_PASSWORD` secrets it signs `release.toml` and the zip; it adds `SHA256SUMS` and publishes every file (a second run for the same tag replaces the files). Without the key the release is published unsigned with a warning, and the updater refuses it.
- Repository variables `HUEDRA_UPDATE_KEY` and `HUEDRA_UPDATE_URL` go into the build when set.
- Actions pinned to commit hashes; the workflow token reads only, except `release`, which writes contents.
