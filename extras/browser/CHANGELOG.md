# Changelog — browser extension

The extension's version is separate from the program's; it is set in `manifest.json` and packed by `build/pack-extension.ps1`.

## Unreleased

### Changed
- New icons: the visible colour gamut with the BT.2020 triangle, matching the program.

## 0.3.0 — 2026-10-31

### Added
- Icons at 16, 32, 48 and 128 pixels, drawn by the same code as the program's icon.
- `minimum_chrome_version` 111 and the Firefox data-collection declaration (`browser_specific_settings.gecko`).
- Store listing notes in `CHROMEWEBSTORE.md` (not packed).

## 0.2.0 — 2026-10-31

### Added
- Streaming sites (Netflix, Disney+, Prime Video, HBO Max, Apple TV): the tab title is marked while a video plays in full screen, for the optional streaming-site rule in Huedra.

## 0.1.0 — 2026-10-31

### Added
- YouTube: the tab title is marked while an HDR video is open, and the player picks the HDR stream again after Huedra turns HDR on.
