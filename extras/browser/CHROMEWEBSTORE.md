# Chrome Web Store Listing — Huedra

> Last Updated: 2026-11-03 (0.3.1 published)

What to enter in the Chrome Developer Dashboard (Edge Add-ons and Firefox AMO ask
for the same facts). Kept in the format of Google's `chrome-extensions` guidance.
Not packed: `build/pack-extension.ps1` packs only the files the manifest names.

## Store Listing

**Extension Name**
Huedra

**Short Description** (69 of 132 characters; same as the manifest)
Tells Huedra when an HDR video is open, so it can turn HDR on for it.

**Detailed Description** (plain text; the store strips Markdown)

```
Turns Windows HDR on while you watch HDR video in your browser, and back off when you stop. Works together with Huedra, a Windows app that switches HDR for games and video.

Features
YouTube: when the video you open is available in HDR, Huedra turns HDR on. When the display switches, the video is loaded again at the same moment, so it can play its HDR stream.
Netflix, Disney+, Prime Video, HBO Max and Apple TV: these sites do not show whether a video is in HDR, so you choose in Huedra which of them count. On a chosen site, a full-screen video turns HDR on, and leaving full screen turns it off again.
Nothing to set up in the browser: the extension has no options and no buttons.

How to use it
1. Install Huedra on Windows and start it.
2. Install this extension.
3. Open an HDR video. Huedra turns HDR on, and off again after you close the tab.
For the streaming sites, turn the site on in Huedra: Settings, Video players, Streaming sites.

Privacy
The extension runs only on YouTube and the five streaming sites above. It sends nothing over the network, stores nothing and has no access to other sites. It tells Huedra about an HDR video by adding a short marker to the start of the tab title, which Huedra reads on your PC.

Requires Huedra for Windows. Without it, the extension does nothing you can see except the title marker.

Download Huedra: https://github.com/apixelsnet/huedra/releases
```

**Category**
Tools.

**Single Purpose**
Tells the Huedra Windows app when an HDR video is open in a tab, so it can turn the display's HDR mode on and off.

**Primary Language**
English

## Graphics & Assets

| Asset | Dimensions | Status | Filename |
|-------|-----------|--------|----------|
| Store Icon | 128×128 PNG | ✅ Ready | `icons/icon-128.png` |
| Screenshots 1–5 | 1280×800, 24-bit PNG | ✅ Uploaded | Video players (Streaming sites), Status with HDR on, Games, Displays, General |
| Small Promo Tile | 440×280, 24-bit PNG | ✅ Uploaded | Logo, wordmark, "HDR on for HDR video, off when it ends." |
| Marquee Promo Tile | 1400×560, 24-bit PNG | ✅ Uploaded | Logo, wordmark, the sites as text, the Status page with HDR on |

### Screenshot Notes
The settings window of a portable copy, captured with `PrintWindow` at
1088×680 on a 1280×800 ink-violet canvas with a one-line caption above it.
The promo tiles are HTML rendered by headless Edge. The images are not kept
in the repository (they show the owner's game library and displays).
1. Video players, scrolled to Streaming sites: "Choose the video players and streaming sites that turn HDR on"
2. Status in Always HDR mode: "HDR turns on for games and HDR video, and off again afterwards"
3. Games: "Your games from every store, with their HDR support"
4. Displays: "Choose the displays Huedra manages"
5. General: "Starts with Windows and runs quietly in the notification area"
Other sites' names appear only as text; no logos (store trademark rule).

## Permissions Justification

The manifest asks for no `permissions` and no `host_permissions`. Its content
scripts run on these sites, which the dashboard lists as host access:

| Site | Type | Justification |
|------|------|---------------|
| `https://www.youtube.com/*` | content script | Reads the open video's list of formats to see whether any is HDR, and adds a marker to the tab title while it is, so the Huedra app turns HDR on. When the display switches to HDR, loads the video again at the same time so it plays its HDR stream. |
| `https://www.netflix.com/*` | content script | While a video plays full screen, adds a marker to the tab title so Huedra can turn HDR on if the user chose this site in Huedra. The site does not reveal whether a video is HDR, so full screen is the only signal. |
| `https://www.disneyplus.com/*` | content script | Same as Netflix. |
| `https://www.primevideo.com/*` | content script | Same as Netflix. Amazon's shop domains are left out on purpose. |
| `https://play.hbomax.com/*`, `https://play.max.com/*` | content script | Same as Netflix; HBO Max's player runs on both addresses. |
| `https://tv.apple.com/*` | content script | Same as Netflix. |

## Privacy & Data Use

### Data Collection

**Does the extension collect user data?** No

Nothing is stored or sent anywhere. On the listed sites, the scripts read the
page's own state on the user's PC (the video's formats, whether a video is full
screen) and change only the tab title. Answer "No" to every data type in the
dashboard's form.

| Data Type | Collected? | Transmitted Off-Device? |
|-----------|-----------|------------------------|
| All types in the form | No | No |

### Data Use Certification
- [x] Data is NOT sold to third parties
- [x] Data is NOT used for purposes unrelated to the extension's core functionality
- [x] Data is NOT used for creditworthiness or lending purposes

Firefox: the manifest declares `data_collection_permissions: { required: ["none"] }`.

## Privacy Policy

**Privacy Policy URL**
`https://github.com/apixelsnet/huedra/blob/main/PRIVACY.md`. Its extension part, for reference:

```
Privacy Policy for Huedra (browser extension)

Last updated: 2026-10-31

The Huedra browser extension does not collect, store or transmit any personal data or browsing information. It runs only on YouTube, Netflix, Disney+, Prime Video, HBO Max and Apple TV, reads whether the open video is in HDR or full screen, and adds a short marker to the tab title for the Huedra app on the same PC. Nothing leaves your device.

The extension does not use cookies, analytics or third-party services.
```

## Distribution

**Payment**: Free.
**Visibility**: Public.
**Regions**: All regions.

## Developer Info

**Trader declaration**: Non-trader (a free hobby project; no sales or paid features).
**Contact Email**: set in the dashboard (shown publicly; must be monitored).
**Official URL**: None.
**Homepage URL**: `https://github.com/apixelsnet/huedra`
**Support URL**: `https://github.com/apixelsnet/huedra/issues`
**Mature content**: No.

## Version History

| Version | Date | Changes | Status |
|---------|------|---------|--------|
| 0.3.1 | 2026-11-01 | New icons: the visible colour gamut with the BT.2020 triangle, matching the program. | Submitted 2026-11-02; published 2026-11-03 |
| 0.3.0 | 2026-10-31 | First store version: YouTube HDR detection, streaming-site full-screen rule, icons, Chrome 111 or later, Firefox 128 or later. | Not submitted (superseded by 0.3.1) |

## Review Notes

**Test instructions** (entered in the dashboard):
```
The extension only signals the Huedra Windows app (https://github.com/apixelsnet/huedra) and has no UI of its own. Without the app, the only visible effect is a marker at the start of the tab title (⟦HDR⟧ on an HDR YouTube video, e.g. a video listed as 2160p60 HDR; ⟦Video netflix⟧ etc. while a streaming video plays full screen). That marker is the signal to the app, not a bug.
```
**Remote code**: No.

### Known Issues / Limitations
- Useful only with the Huedra Windows app running; a reviewer without it sees only the title marker. Say so in the dashboard's notes to the reviewer.
- The title marker (`⟦HDR⟧`, `⟦Video netflix⟧`) is visible in the tab title. That is the signal to the app, not a bug.
- Chrome may show "Unrecognized manifest key 'browser_specific_settings'" on the extensions page. The key is for Firefox; Chrome ignores it.

### Rejection History
None yet.
