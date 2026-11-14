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

## Result (2026-11-14) — switching off the tray's thread
Code: `crates/huedra-win/src/switcher.rs`, `set_hdr_many` in `crates/huedra-win/src/display.rs`, `apply` and `on_switched` in `crates/huedra/src/app.rs`.
- The tray hands each set of switches to a worker thread (`Switcher`) and gets the result back as `Event::Switched`, so the menu, the shortcuts, the pipe and the settings window's status keep answering while Windows changes a display. Before, `set_hdr` ran on the message loop: about 0.9 s per display here, and up to 7 s with the default two retries when a display refused.
- `set_hdr_many` sends every display's request before waiting for any, so two displays switch together instead of one after the other. Retries and the 2-second wait per attempt are unchanged.
- The log says how long each switch took: "Display 1 (…) switched to HDR in 864 ms." A failure says after how long.
- Kept contracts: `huedra exit` waits for switches under way (at most 15 s), then returns the displays to SDR before it answers; `huedra launch` answers once HDR has switched, so the game still starts in HDR; `owned.toml` is written before a switch is queued. A display change that arrives during a switch is read once the switch has finished, so a switch in progress is not taken for a change made outside Huedra.

Verified on this PC (one HDR display connected, portable test copy): `huedra status` answered in 11 ms during an 849 ms switch, and `mode hdr` in 39 ms; HDR on in 849–908 ms and off in 895–908 ms (log lines); `huedra launch notepad.exe` from SDR started Notepad 965 ms after the command, after the 864 ms switch; `huedra exit` sent 100 ms after `mode hdr` answered after 1189 ms with the display back in SDR.
Later the same day, with both displays on: Always HDR turned both on together in 696 ms and Automatic both off in 577 ms (one log line each, same time), against about 0.9 s per display one after the other before.
Not verified: a display refusing the change (the failure path through `on_switched`).

## Result (2026-11-14) — SDR content brightness
Code: `SdrBrightness` in `crates/huedra-core/src/display.rs`, `sdr_brightness` and `set_sdr_brightness` in `crates/huedra-win/src/display.rs`, `hdr brightness` in `crates/huedra/src/hdr.rs` and `crates/huedra-core/src/command.rs`, the brightness box on the Displays page (`crates/huedra-ui/ui/pages.slint`, `set_brightness_later` in `crates/huedra-ui/src/app.rs`).
- Reading uses the documented `DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL` (11); `SDRWhiteLevel` 1000 is 80 nits. Setting uses the request Windows Settings' slider sends, type 0xFFFFFFEE with the level and a "final value" byte; it is not documented, so every change is read back with the documented request and reported as failed when Windows keeps the old level. Range 80 to 480 nits, as in Windows Settings.
- Windows refuses the change (error 50) while the display is in SDR, and reads 80 nits then whatever the setting is, so the value is read and changed only while HDR is on: `huedra hdr brightness` says "HDR off" for a display in SDR, and the Displays page shows **SDR content brightness (nits)** only on a display in HDR. `Display` carries `sdr_brightness`, read with the other display details while HDR is on.
- The page's box sends changes through one worker thread that keeps only the newest value per display, so holding an arrow key applies in order; the display list is now updated in place, so the box keeps its focus through the page's once-a-second display reading (before, the row was rebuilt and lost focus after a few steps).
- CLI: `huedra hdr brightness` lists HDR displays; `huedra hdr brightness 240 [--display N | all]` sets it (main display by default).

Verified on this PC (display 1 in HDR): `hdr brightness` read 160 nits (the value set in Windows); 320 and 120 were set and read back; 500 and 79 were refused with the range message; in SDR the change was refused with a clear message. On the Displays page the box showed 160; Up 20 times gave 180 nits and Down 20 times 160 (read back with the CLI). The display was left at 160 nits.
Not verified: a second display; a display that ignores the request.

Later the same day on display 2 (LG ULTRAGEAR, HDR on through the CLI): `hdr brightness --display 2` read 240 nits, 200 was set and read back, 240 restored, HDR off again.
