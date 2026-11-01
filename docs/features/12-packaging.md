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
- `pack-release.ps1` reads the version from `huedra.exe --version` and writes `dist\release\<version>\`: `huedra.exe`, `release.toml` (LF, no byte-order mark) and `huedra-<version>-windows-x64.zip` (program, README, SECURITY, CHANGELOG, PRIVACY and LICENSE). With `-SecretKey` it signs the manifest and the zip with minisign.
- Start-up entry: written on the first start (no settings file yet) of a non-portable copy, because Start with Windows is on by default; rewritten when the entry names a program that no longer exists. Portable copies never touch it.
- `uninstall` now names both data folders (roaming and local).
- README follows Best-README-Template, with the questions from the plan (anti-cheat, what leaves the PC, SmartScreen, the black flash, removal, tested hardware).

Performance targets, measured on this PC (Windows 11 build 26300):

| Target | Result |
| --- | --- |
| Start-up to tray icon ≤ 100 ms | 20–46 ms from process creation (log line); 119 ms on the very first run of a newly copied exe (antivirus scan) |
| Library scan ≤ 200 ms | 3–4 ms; 91 ms on that first run |
| Settings window ≤ 300 ms | 38–51 ms to the first turn of its event loop (hidden desktop, 4 runs) |
| Tray memory ≤ 4 MB | 1.5 MB private working set, 2 threads, about 12 minutes after start; 1.98 MB private working set (2.87 MB private bytes, 5 threads, 196 handles) after 8 h 17 min |
| Idle wake-ups: none between events | PC untouched (2026-11-01, raw per-thread counters, read each minute): 12 context switches in 10 minutes, none in 6 of the 10 minutes, 0 ms CPU; 156 ms CPU in 8 h 17 min. The few switches are Windows thread-pool threads, not the message loop. (An earlier run during other tests saw about 995.) |
| Switch request ≤ 50 ms | 2 ms (mode Always HDR, both displays on) and 0 ms (hold time over, both back to SDR); the `mode hdr` command returned after 700 ms, which includes Windows switching both displays |
| Executable | 9,883,136 bytes without an update key, about 9.94 MB with one (limit 10,000,000) |

Verified on this PC: a non-portable copy's first start wrote `HKCU\...\Run\Huedra` with its quoted path and logged "Huedra will start with Windows."; `huedra uninstall` closed it and left neither the Run value nor the `exefile` menu key; no Huedra process remained. `pack-release.ps1` without a key packed a test release (zip 5.2 MB, manifest read back by the updater after signing in the build container).
Not verified: `pack-release.ps1 -SecretKey` with minisign on this PC; a release from a fresh clone. (Idle, memory after an hour and switch timing were measured on 2026-11-01, above.)

Note: `uninstall` and `update` act on whichever tray owns the per-user pipe, including a copy started from another folder.

Status: done except the open measurements above; publishing waits for licence and hosting.

## Result (2026-11-01) — GitHub Actions
Code: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `build/check-changelog.ps1`, `extras/browser/CHANGELOG.md`.
- Same shape as the owner's Mango project: `ci.yml` checks pushes to `main` and pull requests and publishes nothing; `release.yml` publishes, started by a pushed tag `vX.Y.Z` or by Run workflow (patch, minor, major or an exact version), which tags the tip of `main` itself. The version comes from the tag and is written into `Cargo.toml` for that build only; the first release without tags takes the `Cargo.toml` version. `release.yml` calls `ci.yml` (`workflow_call`) with the version and tag, so a release runs every check.
- `build` (Ubuntu): builds `build/Dockerfile` (layers kept in the Actions cache) and runs `build/ci.sh` in it, as `remote-build.sh` does on the server. Two Actions caches, written only by pushes to `main`: downloads (Cargo registry, Microsoft CRT and SDK files) and `target` without its incremental and doc folders. The runner's unused SDKs are deleted first: the first run filled the disk while packing a single cache of everything. Fails when `ci.sh` had to change `Cargo.lock`. Uploads `huedra.exe`.
- `windows` (Windows Server 2025): `huedra.exe --version` must print the expected version and an unknown option must exit 2; changelogs (`check-changelog.ps1`): `CHANGELOG.md` needs an Unreleased section, and for a release a section for the version or a non-empty Unreleased one; `extras/browser/CHANGELOG.md` needs a section for the extension's version. Then `pack-release.ps1` and `pack-extension.ps1`. Uploads the release folder and the extension zip (kept 7 days).
- `release.yml` `publish`: the program must be the tag's version; the notes are that version's section of `CHANGELOG.md`, else its Unreleased section, followed by GitHub's generated list; with the `MINISIGN_SECRET_KEY` and `MINISIGN_PASSWORD` secrets it signs `release.toml` and the zip; it adds `SHA256SUMS` and publishes every file (a second run for the same tag replaces the files). Without the key the release is published unsigned with a warning, and the updater refuses it.
- Repository variables `HUEDRA_UPDATE_KEY` and `HUEDRA_UPDATE_URL` go into the build when set.
- Actions pinned to commit hashes; the workflow token reads only, except `release`, which writes contents.

Verified on GitHub (run of commit cc3b1f2): `build` green in 22 min from empty caches (96 tests; every `ci.sh` step); `windows` green in 18 s (`--version` on Windows Server 2025, changelog check, both packs, artifacts uploaded). The cache save failed for lack of disk space, fixed as above: the next run (commit 59f7e76) was green in 17 min and saved both caches (build output 4.4 GB on disk, 1.04 GiB stored; downloads 299 MiB); the Docker image came from its layer cache.
With warm caches (commit 94f6df5): `build` 5 min 46 s, `windows` 25 s, both green.
Not verified: `release.yml` (no tag pushed yet); signing in Actions (no secret set).

## Result (2026-11-01) — licence and signing key
- Licence: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `license` in the Cargo metadata); both files and `PRIVACY.md` go into the release zip.
- Release signing key made with minisign 0.12 on the owner's PC (password-protected, kept outside the repository). The repository variables `HUEDRA_UPDATE_KEY` (its public key) and `HUEDRA_UPDATE_URL` (the GitHub releases address) are set, so Actions builds check for updates. The secrets `MINISIGN_SECRET_KEY` and `MINISIGN_PASSWORD` are set by the owner.
- minisign on Windows reads the password from a pipe (checked with a throwaway key), as the release job does on Linux.

## Result (2026-11-01) — first release, 0.1.0
- Started with Release > Run workflow (version 0.1.0): the workflow tagged `v0.1.0` on `main`, ran every CI check with that version, signed and published "Huedra 0.1.0" with the zip (5,358,375 bytes), `huedra.exe` (9,944,576 bytes, update key built in), `release.toml`, both signatures, the extension zip (0.3.0) and `SHA256SUMS`. Notes: the 0.1.0 section of `CHANGELOG.md`, then GitHub's list.
- Verified on this PC from the downloaded files: both signatures verify with the release public key (trusted comment "huedra 0.1.0"); every SHA-256 in `SHA256SUMS` matches; `release.toml` names the exe's size and digest; `huedra.exe --version` prints 0.1.0; a portable copy of the release ran `huedra update --check --from` on the release files and answered "Huedra 0.1.0 is up to date", so the program's own signature check accepts the signature made in Actions.
- Not verified: a download through the release address (the repository is private, so the address answers "not found" to the program); an update from 0.1.0 to a later release.

## Result (2026-11-01) — one licence, betas, fewer downloads
- Licence: Apache-2.0 only, chosen by the owner over the MIT/Apache pair (`LICENSE`; `license = "Apache-2.0"`). README: licence, contribution terms (section 5) and a Disclaimer section (beta, no warranty, display settings, anti-cheat, no affiliation, game information).
- Releases before 1.0.0 are GitHub pre-releases titled "Huedra x.y.z beta"; 1.0.0 and later are normal releases.
- A version's release holds only what people download: the program zip, its signature and `huedra-browser-extension-<version>.zip`. The notes start with a Downloads table (which file to take) and the minisign command with the public key, then the changelog section. `SHA256SUMS` is gone: GitHub shows each file's SHA-256.
- The updater's files (`huedra.exe`, `release.toml`, `release.toml.minisig`) go to the fixed pre-release `update-channel`, program first and manifest last. `HUEDRA_UPDATE_URL` points there, because `releases/latest/download/` skips pre-releases.
- 0.1.0, first published as a normal release with the `latest` address built in, was withdrawn and published again as a beta.
- Release notes list only what matters to people using Huedra; the project's own build and workflow changes stay out of `CHANGELOG.md`.

## Result (2026-11-01) — who may change what ships
Code: `build/protect-repo.sh`.
- Set now: workflows get a read-only token unless a job asks for more (only the Release workflow's `version` and `publish` jobs write). Running the Release workflow needs write access to the repository, which only its admins have; pull requests come from forks.
- Waiting for a public repository (GitHub offers repository rules on a free organisation only for public repositories; the API answered "Upgrade to GitHub Pro or make this repository public"): `main` only through a pull request with one approval and both CI checks green (admins may push directly), no force push or deletion; tags `v*` and `update-channel` changed only by admins and the Release workflow; outside contributors' workflow runs wait for an admin. `bash build/protect-repo.sh` applies all of it once the repository is public.

## Result (2026-11-01) — program icon as PNG, extension icons from CI
- `build.rs` stores the 256-pixel image of the program icon as PNG (Windows reads PNG at that size) instead of a 262 KB bitmap; `png` 0.18 was already in the dependency tree. The build with the disclaimer, the About actions and the update key had reached 10,064,384 bytes; with the PNG it is 9,838,080.
- When the committed extension icons differ from what the code draws, `ci.sh` keeps the new ones and CI uploads them as the artifact `extension-icons`, so icons can be updated without a local build.
- README rewritten for users (logo, tour, features, quick start, questions, support, licence, disclaimer); build and contribution notes moved to `CONTRIBUTING.md`; `docs/USER-GUIDE.md` for every setting and task; `.github/FUNDING.yml` shows the Sponsor button (Ko-fi). The README's animated logo (`docs/assets/huedra-logo.svg`) is drawn from the icon's own colours and animates with CSS, and stays still for viewers who ask for reduced motion; the tour (`docs/assets/huedra-tour.gif`, 1.7 MB) and the Games screenshot were taken on a hidden desktop from a portable test copy with default settings.

## Result (2026-11-01) — issues, labels and branches
Code: `.github/ISSUE_TEMPLATE/`, `.github/labels.yml`, `.github/release.yml`, `.github/workflows/labels.yml`, `changelog.yml`, `triage.yml`; CONTRIBUTING.md.
- Issue forms (blank issues off): Bug report, Game or display problem, Feature request, Documentation, Question; links for private security reports, the user guide and the feature docs. Bug and compatibility forms ask for the version, Windows, graphics card, displays and the log.
- 37 labels in `labels.yml`: kind, status, priority, area and changelog. The Labels workflow creates and updates them on a push to `main` that changes the file. The Triage workflow adds the area labels picked in a form (area names contain no commas, since several picks arrive joined by ", "). The Changelog workflow labels a pull request by its title (feat, fix, other), and `release.yml` sorts the generated release notes by those labels.
- Branches are named `feat/`, `fix/`, `docs/`, `perf/`, `refactor/`, `test/`, `ci/` or `chore/`; commit messages and pull request titles follow Conventional Commits (CONTRIBUTING.md).
- README disclaimer: a note that parts of the project were written with the help of AI tools and are reviewed and tested by the maintainer.

## Result (2026-11-01) — artifact retention
- Every artifact the CI workflow uploads (`huedra-exe`, `huedra-release`, `huedra-extension`, `extension-icons`) is kept for 1 day instead of 7, and so is the Docker build record that `docker/build-push-action` uploads (`DOCKER_BUILD_RECORD_RETENTION_DAYS`; it had 90 days). The Release workflow reads them within the same run, so it is unaffected. Logs keep the repository default.

## Result (2026-11-01) — supporters
- `SUPPORTERS.md` lists Ko-fi supporters who ask to be named; the README's Supporters section says what supporters get: their name there, thanks in release notes, preview builds through Ko-fi about a week before each public release, and the `sponsor` label on their issues. No votes or priority, and no feature is locked.
- The Release workflow ends the GitHub release notes with a Thank you section linking `SUPPORTERS.md`; the signed notes for the update dialog are unchanged.
- `.github/labels.yml`: `sponsor` label (pink). `.github/FUNDING.yml` already shows the Ko-fi button; no GitHub Sponsors entry yet.

## Result (2026-11-01) — essential CI runs only
Owner: only the needed runs, and only essential steps (the repository is private, so Actions minutes are paid; builds go through `build/remote-build.sh`).
- `ci.yml` runs on a push to main or a pull request only when a build input changed (`crates/`, `extras/browser/`, Cargo files, `deny.toml`, `clippy.toml`, `.cargo/`, the build scripts, `ci.yml`). A push to main runs the build job alone: no Windows job, no `huedra-exe` upload.
- Pull requests from branches of this repository skip both jobs (shown as skipped, which the branch rules accept); pull requests from forks run both. Releases (also when started by a tag push, told apart by the version) and manual runs run both.
- `docker/build-push-action` no longer uploads its build record or writes a job summary (`DOCKER_BUILD_RECORD_UPLOAD`, `DOCKER_BUILD_SUMMARY` false).
- `changelog.yml` runs on an edit only when the title changed.
- A documentation-only pull request from a fork gets no CI run; the required checks then wait, and an admin merges it with the bypass.

## Result (2026-11-01) — third-party licences
Code: `build/licences.sh`, `build/about.toml`, `build/about.hbs`, `build/Dockerfile` (cargo-about 0.9.2, `--features cli`), `build/ci.sh`, `build/remote-build.sh`, `build/pack-release.ps1`, `.github/workflows/ci.yml`; `THIRD-PARTY-LICENSES.md`.
- `ci.sh` runs `licences.sh`, which writes `THIRD-PARTY-LICENSES.md` in the source tree with cargo-about (`--offline`: texts come from the crates, so every machine writes the same file), for the Windows target without build and dev dependencies. Like `Cargo.lock`, `remote-build.sh` copies it back and CI fails when the committed copy differs. `pack-release.ps1` puts it in the release zip.
- Slint: cargo-about 0.9.2 and 0.9.3 drop the text of `LicenseRef-` licences even when clarified (the filter for them is inverted), so `licences.sh` appends the royalty-free licence from Slint's own crate, with the Slint crates in the Windows dependency tree (`cargo tree`). Its SHA-256 is pinned: a changed text fails the build until it has been read. cargo-about still warns "has no license file" for those crates.
- The accepted licences match `deny.toml`.

Verified: on the build server, `licences.sh` writes the file (MIT 189 crates, Unicode-3.0 26, Apache-2.0 7, BSD-3-Clause 2, BSL-1.0 2, Zlib 2, BSD-2-Clause 1, and 9 Slint crates).
Not verified: the CI run on GitHub; the zip from `pack-release.ps1` with the file in it.
