<div align="center">
  <h1>Huedra</h1>
  <p>Turns Windows HDR on while games and HDR video run, and back off when they end.</p>
  <p>
    <a href="#getting-started">Download</a> ·
    <a href="SECURITY.md">Report a security problem</a> ·
    <a href="#roadmap">Roadmap</a>
  </p>
</div>

> **Status:** release candidate. Everything below works on the developer's PC; the first public release waits for a licence and a release location. There is nothing to download yet.

## Table of contents

- [About](#about)
- [Built with](#built-with)
- [Getting started](#getting-started)
- [Usage](#usage)
- [Questions](#questions)
- [Building from source](#building-from-source)
- [Roadmap](#roadmap)
- [Licence](#licence)
- [Acknowledgements](#acknowledgements)

## About

Huedra is a small Windows tray application. It watches for games and HDR video, turns HDR on for the display showing them, and returns that display to SDR when they end. It only ever turns off HDR that it turned on itself.

- One executable, nothing to install, no runtime.
- No background activity between events: it waits for Windows to report a window coming to the front, a title changing or a program closing.
- Games from Steam, Epic, GOG, Xbox, Ubisoft Connect, EA app and Rockstar are found on their own.
- HDR video in mpv, VLC, MPC-HC, MPC-BE and PotPlayer, and on YouTube through the Huedra browser extension.
- "Start with HDR": HDR turns on before a game starts, for games that misbehave when HDR changes while they run.
- Signed automatic updates, with the previous version kept.

## Built with

- [Rust](https://www.rust-lang.org) and [windows-rs](https://github.com/microsoft/windows-rs)
- [Slint](https://slint.dev) for the settings window
- [minisign-verify](https://github.com/jedisct1/rust-minisign-verify) for update signatures

## Getting started

Requirements: Windows 10 version 1903 or later, or Windows 11, on x64, and a display that supports HDR.

1. Download `huedra-<version>-windows-x64.zip` from the release page and unpack it into a folder of your own, for example `%LOCALAPPDATA%\Programs\Huedra`. The folder must stay writable for updates.
2. Optionally verify the download with [minisign](https://jedisct1.github.io/minisign/) and the public key published with the release:
   ```powershell
   minisign -Vm huedra-<version>-windows-x64.zip -P <public key>
   ```
3. Start `huedra.exe`. Windows SmartScreen may warn on first start, because the program is not code-signed yet (see [Questions](#questions)).

On first start Huedra adds itself to Windows start-up (Settings, General turns this off), scans the game stores and starts in Automatic mode. Nothing else needs setting up.

## Usage

Huedra runs in the notification area: a full-colour dot while HDR is on, a grey ring in SDR. Right-click the icon for the menu; left-click it to open the settings window.

| Keys | Action |
| --- | --- |
| Win+Alt+H | Turn HDR on or off |
| Win+Alt+O | Change mode (Automatic, Always HDR, Always SDR) |
| Win+Alt+P | Pause or resume automatic switching |

For a Steam game, Settings › Games shows a launch option that starts the game with HDR already on:

```text
"C:\path\to\huedra.exe" launch -- %command%
```

Command line:

```text
huedra                       Start Huedra in the notification area.
huedra settings [page]       Open the settings window (status, games, video, displays, general).
huedra status                Show what the running Huedra is doing.
huedra games                 List the games found in every store.
huedra mode automatic|hdr|sdr
huedra pause [minutes]       Pause automatic switching (default 60).
huedra resume
huedra toggle                Turn HDR on or off, as Win+Alt+H does.
huedra launch -- <program> [arguments]
huedra hdr status            List the displays and their HDR state.
huedra hdr on|off [--display N | all]
huedra update [--check]      Install a newer signed release now, or only report it.
huedra update --rollback     Go back to the version before the last update.
huedra uninstall             Close Huedra and remove its registry entries.
huedra exit
```

Settings are kept in `%APPDATA%\Huedra\config.toml`; the log, the recovery record and caches in `%LOCALAPPDATA%\Huedra`. A file named `portable` next to `huedra.exe` keeps everything in a `data` folder beside it instead, and leaves Windows start-up alone.

## Questions

**Can this cause problems with anti-cheat software?**
It should not. Huedra reads the name and path of the program in front and waits for programs to close. It never reads or writes another program's memory and never places code in another program.

**What information leaves this computer?**
Two things, both on by default and both can be turned off under Settings › General:
- Update checks: once a day, the release files are downloaded from the release location. No identifier is sent beyond a `Huedra/<version>` user agent.
- Game information: the settings window looks up installed games on [PCGamingWiki](https://www.pcgamingwiki.com), sending each game's Steam ID, or its name for other stores, at most once per second, and downloads Discord's public list of games for icons. Results are kept for 30 days.

There is no telemetry. Window titles are not written to the log unless you turn that on.

**Why does SmartScreen show a warning?**
The program has no Windows code-signing certificate yet. Updates are still checked: each one must carry a valid signature from the project's own key before it is installed.

**Why does the screen turn black briefly when switching?**
That is Windows changing the display mode; it takes one to two seconds and is the same when HDR is switched in Windows Settings.

**How is it removed completely?**
Run `huedra uninstall`: it closes Huedra and removes the start-up entry and the "Start with HDR" menu entry. Then delete the program folder and `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra`. Desktop shortcuts made by Huedra are ordinary files.

**Which graphics cards and Windows versions have been tested?**
Windows 11 (build 26300) with an NVIDIA card and two HDR displays. AMD and Intel graphics, Windows 10 and laptops are not tested yet.

## Building from source

Huedra is cross-compiled for Windows on a Linux machine with Docker, reached over SSH.

1. Name the build server (this file is ignored by Git):
   ```sh
   echo "HUEDRA_BUILD_HOST=<server>" > build/local.env
   ```
   For a build that checks for updates, add `HUEDRA_UPDATE_KEY=<minisign public key>` and `HUEDRA_UPDATE_URL=https://<release folder>/`.
2. Build and test on the server, and copy `huedra.exe` to `dist/`:
   ```sh
   bash build/remote-fmt.sh     # optional: format the sources with rustfmt
   bash build/remote-build.sh
   ```
3. Run it on Windows:
   ```powershell
   powershell -File build/run.ps1 --version
   ```
4. Pack a release (zip, `release.toml`, signatures):
   ```powershell
   powershell -File build/pack-release.ps1 -SecretKey <path to minisign secret key>
   ```

### GitHub Actions

`.github/workflows/ci.yml` runs the same checks on every push and pull request: `build/ci.sh` in the `build/Dockerfile` container, then `huedra.exe --version` on a Windows runner, the changelog check (`build/check-changelog.ps1`), and the release and extension packs. The run's artifacts hold `huedra.exe`, the release folder and the extension zip.

To publish, add a `## <version>` section to [CHANGELOG.md](CHANGELOG.md) and push the tag `v<version>`. The workflow creates the GitHub release with that section as its notes. Optional settings:

| Kind | Name | Use |
| --- | --- | --- |
| Variable | `HUEDRA_UPDATE_KEY`, `HUEDRA_UPDATE_URL` | Build the update check in |
| Secret | `MINISIGN_SECRET_KEY`, `MINISIGN_PASSWORD` | Sign `release.toml` and the zip (contents of the key file, and its password) |

## Roadmap

Each feature has a short document in [`docs/features`](docs/features), stating its scope and what has been built and verified. Changes are listed in [CHANGELOG.md](CHANGELOG.md).

## Licence

Not yet chosen. Until a licence is added, all rights are reserved.

## Acknowledgements

- [SmartHdrManager](https://github.com/VantaCode420/SmartHdrManager) by VantaCode420, for the original idea.
- [PCGamingWiki](https://www.pcgamingwiki.com), for game information (CC BY-NC-SA 3.0).
- [Slint](https://slint.dev), for the settings window, used under its royalty-free licence.
