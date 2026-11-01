# Contributing to Huedra

Thank you for helping. Bug reports, testing on hardware the project has not seen yet, and pull requests are all welcome.

## Reporting a problem

[Open an issue](../../issues/new/choose) and pick the form that fits: **Bug report**, **Game or display problem**, **Feature request**, **Documentation** or **Question**. The forms ask for what is needed to find a problem:

- the Huedra version (`huedra --version`) and the Windows version;
- the graphics card and the displays involved;
- what happened and what you expected;
- the relevant lines of the log (`%LOCALAPPDATA%\Huedra\huedra.log`).

Issues are labelled by kind (`bug`, `enhancement`, `compatibility`…), area (`area: games`, `area: video`…), status and priority. A maintainer confirms new issues; `good first issue` marks ones suited to a first contribution.

Report security problems privately, as described in [SECURITY.md](SECURITY.md), not in a public issue.

## Project layout

| Path | Contents |
| --- | --- |
| `crates/huedra-core` | Rules, the switching engine, configuration and parsers. No Windows code; its tests run on any platform. |
| `crates/huedra-win` | Every Windows call, and all `unsafe` code. |
| `crates/huedra-ui` | The settings window ([Slint](https://slint.dev)), which runs as its own process. |
| `crates/huedra` | The executable: the tray, the settings role and the command line. |
| `extras/browser` | The browser extension. |
| `docs/features` | One short document per feature: its scope and what has been built and verified. |

## Building

Huedra is cross-compiled for Windows from Linux in Docker (`build/Dockerfile`, with `cargo-xwin`). `build/ci.sh` runs every check: formatting, Clippy for Linux and Windows, `cargo deny`, tests, documentation and the Windows release build.

- **With a Linux machine running Docker, reached over SSH:** put `HUEDRA_BUILD_HOST=<server>` in `build/local.env` (ignored by Git), then run `bash build/remote-build.sh`. It copies `huedra.exe` to `dist/`. `bash build/remote-fmt.sh` formats the sources.
- **Without one:** open a pull request from your fork. GitHub Actions runs the same checks when it changes code or the build (not for documentation only), and the run's artifacts contain `huedra.exe`, the release zip and the browser extension.

## Branches and commits

Work on a branch named after the kind of change, in lower case with hyphens:

| Prefix | For |
| --- | --- |
| `feat/` | A new feature, for example `feat/per-display-sdr-lock` |
| `fix/` | A bug fix, for example `fix/hdr-stays-on-after-exit` |
| `docs/` | Documentation only |
| `perf/` | Size, memory or speed |
| `refactor/` | Code changes that do not change behaviour |
| `test/` | Tests only |
| `ci/` | Workflows and the build |
| `chore/` | Dependencies and housekeeping |

Commit messages and pull request titles follow [Conventional Commits](https://www.conventionalcommits.org/): `feat: per-display SDR lock`, `fix: keep HDR off after an outside change`. The title decides the section of the release notes the change is listed in: `feat` under Added, `fix` under Fixed, anything else under Changed.

## Pull requests

- Read the feature's document in [`docs/features`](docs/features) before changing it, and add a dated Result section describing what changed and how it was checked.
- Add a line under `## Unreleased` in [CHANGELOG.md](CHANGELOG.md) for changes users will notice; changes to the browser extension go in [extras/browser/CHANGELOG.md](extras/browser/CHANGELOG.md).
- Every pull request must pass the checks in GitHub Actions.
- Code conventions: `unsafe` only in `huedra-win`, each block with a `// SAFETY:` comment; no `unwrap` outside tests; `thiserror` in libraries and `anyhow` only in the executable.
- Huedra runs without administrator rights, admits only the current user to its command channel, installs only signed updates, and never reads or writes other programs' memory. Changes must keep it that way.
- The build and release setup (`.github/` and `build/`) is maintained by the project's maintainers. Pull requests may suggest changes to it; releases are made only by the maintainers.

## Licence

By submitting a contribution, you agree that it is licensed under the [Apache License 2.0](LICENSE), as section 5 of the licence describes.
