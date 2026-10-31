# 05 — Video

Turns HDR on for HDR video in local players and on YouTube.

## Scope
- Players: VLC, mpv, mpv.net, MPC-HC, MPC-BE, PotPlayer. Modes: when HDR video plays, always while open, never.
- Title keywords: HDR, HDR10, Dolby Vision, DoVi, BT.2020 (editable). Optional "full screen only".
- RTX Video HDR: read the NVIDIA setting only; with it on, SDR video can switch too (Core Audio session check).
- YouTube: userscript in `extras/` with `@grant none` that marks the tab title.

## Done when
An HDR10 file played full screen in mpv turns HDR on, and closing it returns to SDR after the hold time.

Status: not started (phase 4).
