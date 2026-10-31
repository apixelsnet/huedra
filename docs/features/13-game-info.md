# 13 — Game information

HDR support and notes for each game, from PCGamingWiki.

## Scope
- Steam: `GET /api/appid.php?appid=N` (redirects to the page), then `GET /w/api.php?action=parse&page=PAGE&prop=wikitext&format=json&formatversion=2`; read `|hdr`, `|hdr notes`, `|ray tracing`.
- Other stores: match by title through the wiki search; a wrong match can be cleared per game.
- Suggestions: `true` or `limited` gives "Enable while running"; `false` gives "Do not change"; `hackable` shows notes only. Never applied automatically.
- Enabled by default; can be turned off in Settings (General). The first-start notice and the README say what is sent.

## Notes
- The wiki Cargo query API refuses anonymous use (checked 2026-10-31).
- Runs only in the settings process. At most one request per second, a User-Agent naming Huedra, results cached 30 days in `gameinfo.toml` in the local app-data folder.
- Content is CC BY-NC-SA 3.0: shown with attribution and a link, only the fields used are stored, never redistributed.

## Done when
Every installed Steam game on this PC gets a result or a clear "not found", within the rate limit.

Status: not started (phase 3).
