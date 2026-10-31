# 05 — Video

Turns HDR on for HDR video in local players and in the browser.

## Scope
- Players: VLC, mpv, mpv.net, MPC-HC, MPC-BE, PotPlayer. Modes: when HDR video plays, always while open, never.
- Title keywords: HDR, HDR10, Dolby Vision, DoVi, BT.2020 (editable). Optional "full screen only".
- RTX Video HDR: read the NVIDIA setting only; with it on, SDR video can switch too (Core Audio session check).
- Browser: a Manifest V3 extension in `extras/browser/` for Chrome, Edge and Firefox. It replaces the planned YouTube userscript.

## Browser extension
- Detection (YouTube): the video counts as HDR when its formats include one with BT.2020 primaries and a PQ or HLG transfer. The test is whether the video has HDR formats, not whether one is playing: YouTube serves HDR streams only while the display reports `(dynamic-range: high)`, so in SDR one never plays.
- After the switch: on a `dynamic-range` change the extension makes the player pick the HDR stream again (quality re-select, reload as fallback).
- DRM sites (Netflix and similar) do not expose stream details. Optional per-site rule: full-screen video on that site counts as HDR.
- Signal, version 1: a fixed marker in `document.title` while an HDR video is open, matched by the existing title watcher. No new role and no new channel.
- Signal, later and only if the title marker proves unreliable: native messaging. The browser starts `huedra native-host`, registered under HKCU for our extension ID only (`allowed_origins`), which relays to the per-user pipe. Never a localhost HTTP server: any web page could reach it.
- Distribution: Chrome Web Store, Firefox AMO (signed, may be unlisted), Edge Add-ons. Chrome keeps off-store extensions only in developer mode.

## Done when
An HDR10 file played full screen in mpv turns HDR on, and closing it returns to SDR after the hold time. An HDR YouTube video turns HDR on, then plays its HDR stream, and closing the tab returns to SDR after the hold time.

Status: not started (phase 4).
