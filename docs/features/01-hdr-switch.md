# 01 — HDR switch

Reads and changes the HDR state of each display. Everything else in Huedra builds on this.

## Scope
- List displays with a stable identity (device path), HDR support and current state.
- Turn HDR on or off per display, with retries when the driver refuses the first request.
- Windows 11 24H2+ interface (`GET_ADVANCED_COLOR_INFO_2`, `SET_HDR_STATE`), falling back to `SET_ADVANCED_COLOR_STATE`.
- Peak brightness from DXGI `IDXGIOutput6::GetDesc1`.
- Recovery record (`owned.toml`): written before Huedra turns HDR on, cleared when it turns it off, read at start to restore a display after a crash.

## Notes
- The 24H2 constants are documented only by third parties. Confirm their values against the SDK header `wingdi.h` before relying on them.
- Huedra only turns off HDR that it turned on itself.

## Done when
`huedra hdr status | on | off --display N` switches the monitor and the reported state matches Windows Settings.

Status: not started (phase 1).
