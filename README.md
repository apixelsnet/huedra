<div align="center">
  <h1>Huedra</h1>
  <p>Turns Windows HDR on while games and HDR video run, and back off when they end.</p>
</div>

> **Status:** in development, phase 0 of 5. There is nothing to download yet.

## Table of contents

- [About](#about)
- [Building from source](#building-from-source)
- [Roadmap](#roadmap)
- [Licence](#licence)
- [Acknowledgements](#acknowledgements)

## About

Huedra is a small Windows tray application written in Rust. It watches for games and HDR video, turns HDR on for the displays you choose while they run, and returns to SDR when they end. It only ever turns off HDR that it turned on itself.

Goals: one executable with nothing to install, no background activity between events, and no access to game memory.

## Building from source

Huedra is cross-compiled for Windows on a Linux machine with Docker, reached over SSH.

1. Name the build server (this file is ignored by Git):
   ```sh
   echo "HUEDRA_BUILD_HOST=<server>" > build/local.env
   ```
2. Build and test on the server, and copy `huedra.exe` to `dist/`:
   ```sh
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
- [PCGamingWiki](https://www.pcgamingwiki.com), for game information.
