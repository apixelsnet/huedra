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

## Result (2026-10-31) — phase 4, first part
Code: `crates/huedra-core/src/video.rs`, `[video]` in `crates/huedra-core/src/config.rs`, `crates/huedra-win/src/window.rs`, video section of `crates/huedra/src/app.rs`, `extras/browser/`.
- Programs: VLC, mpv, mpv.net, MPC-HC, MPC-BE, PotPlayer, and the browsers Chrome, Edge, Firefox, Brave, Opera, Vivaldi, recognized by program file name. A program is watched from the first time it comes to the front until it exits; programs set to Never are not watched.
- Watching uses two out-of-context WinEvent hooks limited to that one process: title changes always, window moves only while an HDR title waits for full screen. Other programs' changes never reach Huedra. Each event re-reads the process's visible top-level windows.
- Players: a keyword must appear as whole words; case, dots, dashes and underscores are ignored (`Movie.Dolby.Vision.mkv` matches `Dolby Vision`, `HDRezka` does not match `HDR`). Browsers count only the extension's marker `⟦HDR⟧`, never keywords.
- Full screen: the window covers its monitor and has no title bar (a maximized window does not count). On by default.
- Modes per program: When HDR video plays (default), While open (players only), Never. The tray's "Never for HDR video in mpv" sets that program to Never.
- One trigger per process (`video:mpv:1234`, label "HDR video in mpv"); start delay and hold from General, or `[video] hold_secs`.
- Extension (Manifest V3, `world: MAIN` content script on www.youtube.com; Firefox 128 or later): marks the title while the open video offers a BT.2020 format with PQ or HLG transfer, follows YouTube's in-page navigation, and reloads the video at the same time when `(dynamic-range: high)` becomes true. No permissions, no network requests, no storage.

Verified on this PC, without any HDR switch (portable copy in Always SDR mode, full screen off): a renamed program as `mpv.exe` started and ended its trigger as its title went HDR → SDR → Dolby Vision → exit; as `chrome.exe`, a title with "HDR" but no marker was ignored, the marker started it, a New Tab title ended it, and exit with the marker ended it. YouTube served an SDR browser 16 BT.2020/PQ formats for an HDR video; the extension's script marked that video's title, removed the marker after in-page navigation to an SDR video, and restored it on Back.
Not verified: the full-screen test and the move hook; a real player; the extension installed in a browser; the HDR stream after the switch; Firefox; any HDR switch from video (the engine path is the games' path).
Not done: nothing more in code. (RTX Video HDR, streaming sites, icons and packaging: see below.)

## Result (2026-10-31) — Video players page
Code: `VideoPage` in `crates/huedra-ui/ui/pages.slint`, `TextBox` in `controls.slint`, video section of `crates/huedra-ui/src/app.rs`, `parse_keywords`, `keywords_text` and `VideoApp::choices` in `crates/huedra-core/src/video.rs`, `VideoSettings::{set_mode, set_keywords_text}` in `config.rs`.
- Words that mark HDR video: one line separated by commas, applied with Enter or on leaving the field; trimmed, empty and repeated entries dropped (case ignored), and an empty line restores the default words. The field then shows the words as stored.
- Only in full screen (switch) and the hold time for video, which follows the default from General until changed here ("Use the default" goes back).
- Players and Browsers, one drop-down per program: When HDR video plays, While open (players only), Never. The default is not written to the file. A program keeping HDR on shows "Keeping HDR on", from the tray's status.

Verified on this PC, on a separate hidden desktop (nothing shown on the screen, no focus taken; portable copy, no tray): the page in the wide and compact layouts; through UI Automation and posted keys, VLC from Never to While open, full screen off, the hold back to the default and the words ` HDR, hlg ,, hdr, Dolby Vision`, saved as `HDR, hlg, Dolby Vision` with `hold_secs` removed.
Not verified: "Keeping HDR on" (needs a running tray with a video trigger); the light theme.

## Result (2026-10-31) — RTX Video HDR
Code: `rtx_video_hdr` in `crates/huedra-win/src/registry.rs`, `crates/huedra-win/src/audio.rs`, `Assessment::playing` and `VideoKind` in `crates/huedra-core/src/video.rs`, `VideoSettings::sdr_with_rtx`, video section of `crates/huedra/src/app.rs`, Video players page.
- The NVIDIA setting is read, never written: `_User_Global_VAL_TrueHDR` in each NVIDIA adapter's key under `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\NNNN` (adapters found by `ProviderName`); non-zero (5) is on, 0 or missing is off. Not a documented interface; the NVIDIA app and Control Panel write it. Reading needs no elevation. Read at start, on reload and each time a video program comes to the front.
- With it on and "SDR video with RTX Video HDR" on (default), a watched player or browser wants HDR while one of its windows is full screen and a process of the same program has an active audio session on an output device (Core Audio; browsers play sound from helper processes, so the program name counts, not the window's process). Full screen is always required here. HDR titles and the extension's marker still count as before and take precedence. The trigger reads "Video in Chrome with RTX Video HDR". Programs set to Never are not watched.
- Full screen and sound send no window events, so the tray reads the program again every 2 s, only while SDR video can count and a video program is in the foreground or keeps a full-screen window or its trigger. Otherwise no timer runs.
- Video players page: a switch "SDR video with RTX Video HDR", whose description says whether RTX Video HDR is on, off, or no NVIDIA graphics card was found. The Browsers note mentions it.

Verified on this PC (RTX 5080, RTX Video HDR never changed, so the value is missing): the registry read reports off, and the page says so (hidden desktop); the switch wrote `sdr_with_rtx = false` and back; a portable tray in Always SDR mode started and closed normally, with both displays still in SDR. Unit tests cover the rule (full screen, sound, Never, RTX off, HDR title wins).
Not verified: the audio-session check and the 2 s reading in a running tray, and any HDR switch from SDR video; both need RTX Video HDR turned on in the NVIDIA app, which changes the owner's driver setting.

## Result (2026-10-31) — Streaming sites
Code: `StreamingSite` and the site rule in `assess` in `crates/huedra-core/src/video.rs`, `VideoSettings::{sites, set_site}`, `extras/browser/streaming.js` and its manifest entry (extension 0.2.0), Streaming sites on the Video players page.
- Sites: Netflix (`www.netflix.com`), Disney+ (`www.disneyplus.com`), Prime Video (`www.primevideo.com`), HBO Max (`play.hbomax.com`, `play.max.com`), Apple TV (`tv.apple.com`). Amazon's own domains are left out: matching them would give the extension every Amazon shop page.
- Extension: a content script in the extension's own context (not the page's) puts `⟦Video netflix⟧` (the site key) at the start of the tab title while the page is full screen and the full-screen element holds a video with its metadata loaded (open shadow trees included), and removes it after. Paused video still counts, so pausing does not switch the display. Re-applied when the site rewrites the title. No permissions beyond the six hosts, no network requests, no storage.
- Huedra: a browser window whose title carries the marker of a site turned on wants HDR while that window is full screen, whatever "Only in full screen" says; a marked window not yet full screen installs the move hook, so it is caught when it becomes full screen. The trigger is the usual "HDR video in Edge". A program set to Never ignores it. Any page could write the marker into its title; the effect is HDR turning on, the same as the YouTube marker.
- Settings: `[video] sites`, none by default. Video players page, Streaming sites: one switch per site, with its address.

Verified on this PC: unit tests (marker read, chosen site full screen, windowed waits, site not chosen, Never, players ignore markers; config read, write and round trip). The script in headless Edge (nothing on the screen) on a stand-in `www.netflix.com` page served by the test, with a canvas-stream video: marked in full screen, kept through a title rewrite, removed on leaving full screen, not marked full screen without a video, nothing done on another host. The page on a hidden desktop: five switches, off at first; Netflix and HBO Max on wrote `sites`, and turning them off removed the line.
Not verified: the real sites (their players, full-screen element and title changes); Firefox. The owner has no subscription to any of them, so this stays untested here; the extension and the tray path are the ones verified with YouTube (see below). Status of this part: partly done (built and tested on a stand-in page only).

## Result (2026-10-31) — Extension icons and packaging
Code: `crates/huedra-core/examples/extension_icons.rs`, `extras/browser/icons/`, `extras/browser/manifest.json` (0.3.0), `build/pack-extension.ps1`, extension icon step in `build/ci.sh`.
- Icons: the HDR spectrum dot at 16, 32, 48 and 128 pixels (96-pixel dot with 16 pixels clear around it at 128, per store guidelines), drawn by `icon::render`, the same code as the tray and program icon. The example writes them as PNG with no dependencies: `cargo run -p huedra-core --example extension_icons`. CI draws them again and fails when the committed files differ.
- Manifest: `icons`, and `data_collection_permissions: { required: ["none"] }` under `browser_specific_settings.gecko`, which Firefox AMO requires for new MV3 extensions from November 2025. Chrome and Edge ignore the gecko block.
- Package: `powershell -File build/pack-extension.ps1` writes `dist/huedra-extension-<version>.zip` (Git ignores `dist/`) with the manifest at the top and forward-slash entry names; it packs only the files the manifest names and stops if one is missing. The same zip goes to the Chrome Web Store, Edge Add-ons and Firefox AMO.
- Submitting is the owner's: developer accounts (Chrome charges a one-time fee), store listing text and screenshots, and the privacy forms (no data collected, host access only to YouTube and the five streaming sites).

Verified on this PC: CI green with the icon check (the icons drawn on the build server match the committed ones). The 0.3.0 zip (24,877 bytes: manifest, four icons, two scripts) passes Mozilla's `web-ext lint` (version 8) with no errors, warnings or notices. Unpacked from the zip and loaded into headless Edge with `--load-extension` (nothing on the screen): the extension loaded and its manifest-injected script marked a stand-in `www.netflix.com` page in full screen and unmarked it after. The icons look right at 16 and 128 pixels.
Not verified: a store upload or review; the extension in Firefox; the icons in a browser's extensions page.

Checked against Google's `chrome-extensions` guidance (Modern Web Guidance, linked from Chrome's extensions page): Manifest V3, real icon files at their sizes, no permissions, content scripts only on the listed hosts, no remote code, one title write per change (no batched DOM work needed). Added from it: `extras/browser/CHROMEWEBSTORE.md` (listing text, single purpose, a justification per host, privacy answers, draft privacy policy, version history; not packed) and `minimum_chrome_version: "111"` (content scripts in the page's world). Publisher, contact email, privacy-policy URL and screenshots are left to the owner.

## Result (2026-10-31) — Only the display showing the video
Code: `VideoWindow::monitor` and `Assessment::monitors` in `crates/huedra-core/src/video.rs`, `monitor_of` in `crates/huedra-win/src/window.rs`, `assess_video` and `target_for` in `crates/huedra/src/app.rs`.
- Each window's display is read with its full-screen state (the display it overlaps most, by its Windows number). The trigger wants only the displays of the windows that want HDR (for SDR video with RTX Video HDR, the full-screen windows). While video plays, window moves are watched, so HDR follows the window to another display.
- Governed by Displays, "Only the display showing the content" (on by default); off, every managed display switches together as before. A window on a display Huedra does not manage switches nothing.

Verified: unit test (HDR tab on display 2 beside a full-screen window on 1 gives display 2; SDR full screen gives its display; While open gives every window's display). Build green. Earlier, by the owner: an HDR YouTube video turned HDR on, but on both displays (before this change).
Verified on this PC by the owner (YouTube HDR video, full screen, 3 s hold): full screen on display 2 turned HDR on for display 2 only and back to SDR 3 s after leaving full screen; the same on display 1 turned only display 1 on and off. Display 2 stayed in SDR throughout the second run. After the switch YouTube played its HDR stream (the owner saw HDR in the player's quality menu), so the extension's reload on the `dynamic-range` change works in Chrome; the YouTube half of "Done when" is met. An HDR file in MPC-HC, full screen on display 2, turned only display 2 on and back to SDR 3 s after it ended (mpv itself not yet tried).
Not verified: HDR following a window that moves while it stays in HDR (leaving full screen ends the trigger first, so moving a full-screen video did not exercise it).

Status: in progress (phase 4).

## Result (2026-11-01) — extension icons
- The extension's icons are the new gamut icon (see 07), regenerated by CI's `extension-icons` artifact; the 0.3.0 package still carries the old dot until the extension's next version.

## Result (2026-11-01) — extension 0.3.1
- `manifest.json` 0.3.1 for the new icons, so the extension zip of the 0.1.1 release is not a second, different 0.3.0. Store listing history in `extras/browser/CHROMEWEBSTORE.md`.

## Result (2026-11-03) — on the Chrome Web Store
Code: `EXTENSION_STORE_URL` in `crates/huedra-core/src/video.rs`, `open-extension` in `crates/huedra-ui/ui/pages.slint` (Video players › Browsers, **Get the extension**) and `crates/huedra-ui/src/app.rs`.
- 0.3.1 is published on the Chrome Web Store (`kilkdlcbhddpigcdffdnmdknjhlddlod`). The README carries its shields.io version badge, linked to the store page; the users and rating badges answer "not found" until the store has figures.
- `docs/SETUP.md` and `docs/USER-GUIDE.md` install it from the store for Chrome, Edge, Brave, Opera and Vivaldi; Load unpacked stays as the way without the store. Firefox still loads the release zip as a temporary add-on until it is on AMO.
