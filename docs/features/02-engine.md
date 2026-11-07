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

## Result (2026-11-01) — Always HDR and the shortcut on the main display
Code: `Engine::set_manual_target` and `manual_covers` in `crates/huedra-core/src/engine.rs`; `manual_target` in `crates/huedra/src/app.rs`.
- The engine keeps a manual target for Always HDR mode and the "Turn HDR on or off" shortcut. A display outside it follows Automatic mode while Always HDR is on, so a game on the second display still switches it. Turning HDR off with the shortcut still turns every display off.
- The tray sets it from Displays › Always HDR on the main display only (default on): the Windows primary display, or the lowest-numbered managed display when the primary one is not managed; every managed display when the setting is off. It is set again when displays change and when the settings are reloaded. A target that covers no managed display falls back to all of them.

Verified: unit test (shortcut and Always HDR switch only the main display, a game on display 2 still switches it, every display when set to all, changing the setting switches nothing, fallback when the main display is gone). Build green, 99 core tests. On this PC with two HDR displays: Always HDR turned on Display 1 (MO27Q3) only, Display 2 (LG ULTRAGEAR) stayed in SDR, and Automatic returned Display 1 to SDR after the hold time (log, twice).
Not verified on this PC: the shortcut (same engine path as Always HDR), and a primary display that is not managed.

## Result (2026-11-02) — excluding a display that is in HDR
Code: `reload` in `crates/huedra/src/app.rs`.
- Found in the end-to-end pass: with HDR on (or holding) on Display 2 by Huedra, turning off Displays › Manage for it removed the display from the engine, so nothing ever turned it off, not even Exit.
- On reload, a display Huedra owned that is no longer managed and is still connected is returned to SDR, with the reason "Huedra no longer manages this display". A display that was unplugged is left out (switching it would fail).

Verified on this PC through the settings window: Always HDR with every display, then Automatic (display 2 holding), then Display 2 not managed: Display 2 went to SDR; Always HDR afterwards turned on Display 1 only.

## Result (2026-11-07) — Always SDR turns HDR off
Code: `take_over_for_sdr` in `crates/huedra-core/src/engine.rs` (called by `set_mode` and `Engine::new`); test `always_sdr_turns_off_external_hdr`.
- Owner's report: HDR was on, the mode was Always SDR, and HDR stayed on. HDR turned on outside Huedra (Windows Settings, Win+Alt+H, the quick panel's display switch, or already on when Huedra started) was never turned off, not even by Always SDR, while the window promises "Always SDR: HDR stays off".
- Now choosing Always SDR, or starting in it, counts such HDR as Huedra's and turns it off on every display Huedra controls. HDR switched on in Windows afterward, while Always SDR stays selected, is left on, so Huedra never fights the user. Automatic and Always HDR keep the rule: only HDR Huedra turned on is turned off.
- Displays Huedra does not control are never touched.

Verified: unit test (choosing Always SDR turns off external HDR; turning it on again in Windows is left on; starting in Always SDR with HDR on turns it off); CI green.
Not verified on this PC's displays (the test tray was not run with a real switch).
