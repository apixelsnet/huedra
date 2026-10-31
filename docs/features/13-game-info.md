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

## Result (2026-10-31) — phase 3
Code: `crates/huedra-core/src/gameinfo.rs` (paths, parsing, markup cleaning, cache, suggestions), `crates/huedra-win/src/http.rs` (WinHTTP), `crates/huedra-ui/src/lookups.rs` (worker).
- Steam games: `/api/appid.php?appid=N`, following the redirect and reading the final address; others, or a Steam game without a page: `action=query&list=search` by title. Then `action=parse&prop=wikitext&redirects=1`, reading `|hdr`, `|hdr notes` and `|ray tracing`.
- Notes are cleaned of links (internal and external), templates (except `{{term|...}}`), references, emphasis and tags, and cut at 600 characters.
- One request per second at most, on a background thread of the settings process only; User-Agent `Huedra/<version> (...)`; responses over 4 MB are refused. Results, including "not found", are kept 30 days in `gameinfo.toml`; network failures are not stored and are retried next time.
- Suggestions: supported or limited gives "Enable while running", not supported gives "Do not change"; shown as a button, never applied automatically. Games added by hand are not looked up.

Verified on this PC: all 22 store games were looked up; 16 pages were found (for example Resident Evil 4 (2023): limited, with notes; Hades II: no HDR), 6 games show "Not found on PCGamingWiki".
Not verified: behaviour behind a proxy; that the wiki's rate limits are never reached with very large libraries.

Status: done.
