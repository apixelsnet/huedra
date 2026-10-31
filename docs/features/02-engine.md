# 02 — Engine

Decides, per display, when HDR should be on. Contains no Windows code.

## Scope
- States: SDR, waiting (start delay), HDR by Huedra, holding (after the last trigger ends), HDR external.
- Modes: Automatic, Always HDR, Always SDR; pause for a period.
- Per-game and per-player start delay and hold time.
- Never reverts HDR that was turned on outside Huedra.
- Launcher handling: if a tracked process exits within a few seconds, follow a new process from the same folder for up to 45 s.

## Notes
- Tested on Linux with a simulated clock and simulated displays.

## Done when
Every transition in the plan's state diagram has a test, and the engine drives the real display in phase 2.

Status: not started (phase 2).
