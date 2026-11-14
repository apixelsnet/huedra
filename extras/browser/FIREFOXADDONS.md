# Firefox Add-ons Listing — Huedra

> Prepared: 2026-11-14 (0.3.1, not submitted yet)

What to enter on addons.mozilla.org (AMO) to list the extension, so Firefox
users no longer load it by hand after every restart. The texts are the ones of
[CHROMEWEBSTORE.md](CHROMEWEBSTORE.md); this file lists what AMO asks
differently.

## Before submitting

- Package: `build/pack-extension.ps1` (the same zip as the release's
  `huedra-browser-extension-<version>.zip`). Upload it under **Submit a New
  Add-on › On this site**.
- Add-on ID: `huedra@huedra.invalid` (`browser_specific_settings.gecko.id`).
  AMO keeps the ID for good, so it must not change after the first upload. A
  domain the project owns would be nicer; if one is wanted, change the ID before
  the first submission, in `manifest.json` and in the SETUP guide's Firefox
  steps if they mention it.
- `strict_min_version` 128.0: content scripts with `"world": "MAIN"` need
  Firefox 128.
- `data_collection_permissions.required = ["none"]`: AMO asks every new
  extension to declare what it collects; Huedra's collects nothing.
- Run `npx web-ext lint --source-dir extras/browser` first (AMO runs the same
  linter on upload). Result on 2026-11-14 (web-ext 10.8.0, extension 0.3.1):
  0 errors, 0 notices, 2 warnings. Both say `data_collection_permissions` is
  understood only from Firefox 140 and Firefox for Android 142, while
  `strict_min_version` is 128; older versions ignore the key. Raising
  `strict_min_version` to 140.0 (the current ESR) would clear them.

## Listing

| Field | Value |
|-------|-------|
| Name | Huedra |
| Summary (250 characters at most) | Tells Huedra when an HDR video is open, so it can turn HDR on for it. |
| Description | The **Detailed Description** of CHROMEWEBSTORE.md, unchanged. AMO accepts a little Markdown; plain text is fine. |
| Categories | Photos, Music & Videos |
| Support site | https://github.com/apixelsnet/huedra/issues |
| Homepage | https://github.com/apixelsnet/huedra |
| License | Apache License 2.0 |
| Privacy policy | Not needed: it collects and sends nothing. If asked, link `PRIVACY.md` in the repository. |
| Experimental | No |
| Requires payment | No |

## Notes to reviewer

```
The extension adds a short marker to the tab title while an HDR video is open on YouTube, or while a video plays full screen on a streaming site the user chose in the Huedra Windows app. The Huedra app reads window titles on the same PC to turn the display's HDR mode on and off. The extension makes no network requests, stores nothing and asks for no permissions; its source is the unminified youtube.js and streaming.js in the package. Source repository: https://github.com/apixelsnet/huedra (extras/browser).
```

## Source code

The package holds the source as written (no build step, no minifier), so AMO
does not ask for a separate source upload.

## After listing

- Replace the "Firefox loads it by hand" steps in `docs/SETUP.md` (section 7)
  and the **Get the extension** note for Firefox on the Video players page with
  the AMO link.
- Each new extension version is uploaded to AMO as well as to the Chrome Web
  Store.
