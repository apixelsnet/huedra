# 01 — HDR switch

Reads and changes the HDR state of each display. Everything else in Huedra builds on this.

## Scope
- List displays with a stable identity (device path), HDR support and current state.
- Turn HDR on or off per display, with retries when the driver refuses the first request.
- Windows 11 24H2+ interface (`GET_ADVANCED_COLOR_INFO_2`, `SET_HDR_STATE`), falling back to `SET_ADVANCED_COLOR_STATE`.
- Peak brightness from DXGI `IDXGIOutput6::GetDesc1`.
- Recovery record (`owned.toml`): written before Huedra turns HDR on, cleared when it turns it off, read at start to restore a display after a crash.

## Notes
- The 24H2 values (`GET_ADVANCED_COLOR_INFO_2` = 15, `SET_HDR_STATE` = 16) and struct layouts were checked against `wingdi.h` from SDK 10.0.26100. The `windows` crate 0.62 lacks them, so `huedra-win` defines the packets itself.
- State is the HDR user setting (bit 5 of the v2 flags), which is what Windows Settings shows. Bit layouts are decoded in `huedra-core` (`color_flags`) so they are unit-tested on Linux.
- Display numbers come from the GDI source name (`\\.\DISPLAY2` is 2). The primary display is the one whose desktop starts at (0, 0).
- DXGI peak luminance can read wrong for a moment after a mode change (550 instead of 465 nits once). Read it when the display is settled, not straight after a switch.
- Huedra only turns off HDR that it turned on itself.

## Done when
`huedra hdr status | on | off --display N` switches the monitor and the reported state matches Windows Settings.

## Result (2026-10-31) — phase 1
Code: `crates/huedra-win/src/display.rs` (CCD and DXGI), `crates/huedra-core/src/display.rs` (types, selector, flag decoding), `crates/huedra/src/hdr.rs` (command).
- `displays()` lists active displays with device path, number, EDID name, primary flag, support, state and peak; `set_hdr()` sends the request up to three times and waits up to 2 s for Windows to report the new state.
- The 24H2 interface is tried first; `SET_ADVANCED_COLOR_STATE` is used when `GET_ADVANCED_COLOR_INFO_2` is refused.
- Tests: flag decoding (v1 and v2), display selection, nits, labels.

Verified on this PC (Windows 11 build 26300, two monitors): `hdr status` lists both displays with peak brightness; `hdr on --display 1` took 321 ms, `hdr off` 277 ms, and the screen visibly switched; display 2 and `--display all` work; `--display 3` reports that there is no display 3.
Not verified: the v1 fallback (needs Windows 10 or 11 before 24H2); a display limited by policy (laptop on battery); that the GDI numbers always match Settings > Identify. The recovery record (`owned.toml`) comes with the engine in phase 2.

## Result (2026-10-31) — phase 2
Code: `crates/huedra-core/src/files.rs` (`load_owned`, `save_owned`), `crates/huedra/src/app.rs` (`apply`, `shutdown`), `crates/huedra-win/src/display.rs` (`current`, `set_hdr` with an attempt count).
- The recovery record `owned.toml` (local app-data) is written with the union of old and new ownership before HDR is turned on, and with the final set after switching. A missing or corrupt record owns nothing, so Huedra then leaves HDR alone.
- `current()` lists displays without DXGI; peak brightness is read only for `hdr status` (and later the Displays page).
- Attempts per switch come from the settings (one try plus `retry_attempts`, default 2).

Verified on this PC: after a game ran, `owned.toml` listed both displays; `huedra exit` turned both back to SDR and emptied the record.
Not verified: recovery after a real crash (covered by the engine test `recovery_turns_off_owned_hdr_after_hold`).

## Result (2026-10-31) — open items
Code: `crates/huedra-win/src/display.rs` (test `v1_interface_switches_hdr`).
- A Windows-only test drives the interface used before 24H2 (`GET_ADVANCED_COLOR_INFO` and `SET_ADVANCED_COLOR_STATE`) on a real HDR display that is in SDR: on, then off, checking the v1 state and that the v2 state agrees. It changes the screen, so it is `#[ignore]`d and runs only with `--ignored`; a drop guard turns HDR off however it ends.

Verified on this PC: the test passed on display 1 (HDR on through v1 in 230 ms, off in 479 ms; v2 agreed both times; both displays in SDR afterwards).
Not verified: the choice of v1 on a system that refuses `GET_ADVANCED_COLOR_INFO_2` (Windows 10 or 11 before 24H2); only the v1 calls themselves were exercised here. It is on the pre-release checklist (12).

Status: done.
