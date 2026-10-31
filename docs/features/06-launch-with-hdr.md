# 06 — Launch with HDR

Turns HDR on before a game starts, for games that misbehave if HDR changes during start-up.

## Scope
- `huedra launch -- <command>`: turn HDR on, wait until active, start the game, hold HDR until it exits.
- Steam launch option text, desktop shortcuts, "(with HDR)" context-menu entry.
- Arguments go to `CreateProcessW` directly, never through `cmd.exe`.

## Done when
A Steam game started through `huedra launch` starts in HDR, and every registry entry written is removed by `huedra uninstall`.

Status: not started (phase 4).
