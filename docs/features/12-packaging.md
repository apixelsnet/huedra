# 12 — Packaging and build

How Huedra is built, run, installed and removed.

## Scope
- Build on the server in Docker (`build/Dockerfile`, image `huedra-build`); cross-compile with `cargo-xwin` to `x86_64-pc-windows-msvc`.
- `build/remote-build.sh` syncs the source, runs `build/ci.sh` in the container and copies `huedra.exe` to `dist/`.
- `build/run.ps1` stops a running copy and starts the new one.
- Later: release archive, start-up entry (HKCU Run), `huedra uninstall` removing every key Huedra wrote.

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

Status: phase 0 done. Release archive, start-up entry and uninstall come in phase 5.
