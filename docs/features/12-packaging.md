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

Status: phase 0 done. Release archive, start-up entry and uninstall come in phase 5.
