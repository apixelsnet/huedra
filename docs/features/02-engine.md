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

## Result (2026-10-31) — phase 2
Code: `crates/huedra-core/src/engine.rs`, `crates/huedra-core/src/tracker.rs`.
- Phases `Sdr`, `Waiting`, `HdrByHuedra`, `Holding`, `HdrExternal` per managed display; every transition of the plan's diagram is covered by a test with a simulated clock (13 engine tests, 3 tracker tests).
- The engine returns actions, history notes and its next deadline; the tray sets one timer for that deadline and none when idle.
- Decisions taken where the plan was silent: HDR turned on with Win+Alt+H belongs to the user (never turned off automatically); turning HDR off outside Huedra, or with the shortcut, silences the running game until it ends; a refused switch to HDR does not retry until the next trigger; a refused return to SDR retries after 30 s; a mode that would undo the shortcut gives way to Automatic; pausing keeps the current state.
- Launchers: a process that exits within 10 s opens a 45 s window in which a process started later from the game's folder takes over. Only processes that did not exist when the window opened qualify.
- On exit Huedra returns its own HDR displays to SDR.

Verified on this PC with Character Map as a test game (3 s hold): HDR on at start, held 3 s after closing, then SDR with the reason "Character Map closed; hold time of 3 seconds elapsed"; launcher case (first copy closed after 2 s, second copy adopted, HDR stayed on); Always HDR, Always SDR, pause and resume; outside changes reported and left alone.

## Result (2026-10-31) — Per-display triggers
Code: `Target` and `Engine::retarget` in `crates/huedra-core/src/engine.rs`.
- Each trigger carries a target: every managed display, or only some (empty while its window is not known, so nothing switches). A display turns on only for triggers that cover it; its delay, hold and reason come from those triggers alone.
- `retarget` moves a running trigger: the new display turns on, the old one holds for the trigger's hold time and then turns off with the reason "X moved to another display".
- Turning HDR off outside Huedra, or a refused switch to HDR, now silences only the triggers covering that display; the shortcut still silences all.
- Each display remembers the trigger that last kept it on, replacing the single "last ended" record.

Verified: unit test (no window, then one display, moving to the other with hold and reason, a second display's trigger unaffected by the first display being turned off outside). Build green, 88 tests.
Not verified on this PC: a real switch of one display while the other stays in SDR.

Status: done.
