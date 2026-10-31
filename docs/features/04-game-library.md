# 04 — Game library

Finds installed games so that HDR can follow them without configuration.

## Scope
- Steam, Epic, GOG, Xbox, Ubisoft Connect, EA app, Rockstar.
- Manual addition by executable. Per-game mode, start delay and hold time.

## Notes
- Parsers (VDF, ACF, Epic manifests) contain no Windows code and are tested on Linux with sample files. Locating the files is in `huedra-win`.

## Done when
All seven stores list their installed games on this PC in under 200 ms.

Status: not started (phase 3).
