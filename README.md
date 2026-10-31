<div align="center">
  <h1>Huedra</h1>
  <p>Turns Windows HDR on while games and HDR video run, and back off when they end.</p>
</div>

> **Status:** in development, phase 3 of 5: HDR switching, the tray, game libraries and the settings window work; video players, Launch with HDR and updates are still to come. There is nothing to download yet.

## Table of contents

- [About](#about)
- [Usage](#usage)
- [Privacy](#privacy)
- [Building from source](#building-from-source)
- [Roadmap](#roadmap)
- [Licence](#licence)
- [Acknowledgements](#acknowledgements)

## About

Huedra is a small Windows tray application written in Rust. It watches for games and HDR video, turns HDR on for the displays you choose while they run, and returns to SDR when they end. It only ever turns off HDR that it turned on itself.

Goals: one executable with nothing to install, no background activity between events, and no access to game memory.

## Usage

Start `huedra.exe` without arguments. It runs in the notification area: a full-colour dot while HDR is on, a grey ring in SDR. Right-click the icon for the menu; left-click it to open the settings window.

Keyboard shortcuts:

| Keys | Action |
| --- | --- |
| Win+Alt+H | Turn HDR on or off |
| Win+Alt+O | Change mode (Automatic, Always HDR, Always SDR) |
| Win+Alt+P | Pause or resume automatic switching |

Command line:

```text
huedra                       Start Huedra in the notification area.
huedra settings [page]       Open the settings window (status, games, displays, general).
huedra status                Show what the running Huedra is doing.
huedra games                 List the games found in every store.
huedra mode automatic|hdr|sdr
huedra pause [minutes]       Pause automatic switching (default 60).
huedra resume
huedra toggle                Turn HDR on or off, as Win+Alt+H does.
huedra hdr status            List the displays and their HDR state.
huedra hdr on|off [--display N | all]
huedra exit
```

Settings are kept in `%APPDATA%\Huedra\config.toml`; the log, the recovery record and caches in `%LOCALAPPDATA%\Huedra`. A file named `portable` next to `huedra.exe` keeps everything in a `data` folder beside it instead.

## Privacy

Huedra reads the names and paths of programs that come to the front, and nothing else from them: no process memory is read and no code is placed in other programs, so anti-cheat software is unaffected. Window titles are not written to the log.

The settings window looks up installed games on [PCGamingWiki](https://www.pcgamingwiki.com) to show whether they support HDR. It sends each game's Steam ID, or its name for other stores, at most once per second, and keeps the results for 30 days. This can be turned off under General.

## Building from source

Huedra is cross-compiled for Windows on a Linux machine with Docker, reached over SSH.

1. Name the build server (this file is ignored by Git):
   ```sh
   echo "HUEDRA_BUILD_HOST=<server>" > build/local.env
   ```
2. Build and test on the server, and copy `huedra.exe` to `dist/`:
   ```sh
   bash build/remote-fmt.sh     # optional: format the sources with rustfmt
   bash build/remote-build.sh
   ```
3. Run it on Windows:
   ```powershell
   powershell -File build/run.ps1 --version
   ```

## Roadmap

Each feature has a short document in [`docs/features`](docs/features), stating its scope and what has been built and verified.

## Licence

Not yet chosen. Until a licence is added, all rights are reserved.

## Acknowledgements

- [SmartHdrManager](https://github.com/VantaCode420/SmartHdrManager) by VantaCode420, for the original idea.
- [PCGamingWiki](https://www.pcgamingwiki.com), for game information (CC BY-NC-SA 3.0).
- [Slint](https://slint.dev), for the settings window, used under its royalty-free licence.
