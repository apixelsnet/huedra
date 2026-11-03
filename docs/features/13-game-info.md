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
Not verified: behavior behind a proxy; that the wiki's rate limits are never reached with very large libraries.


## Result (2026-10-31) — end-to-end pass
- The ray tracing field is shown in words and left out when the wiki does not say (`GameInfo::ray_tracing_label`, unit test `ray_tracing_in_plain_words`); the cache keeps the raw value.

Verified on this PC: Deadlock shows "Ray tracing: Not supported"; Cuphead (field unknown) shows no ray tracing line.

## Result (2026-10-31) — game icons
Code: `crates/huedra-core/src/icons.rs` (Discord list parsing and matching, the index, Steam's cached icon, finding a game's program), `crates/huedra-core/src/library/xbox.rs` (`logo_files`), `crates/huedra-win/src/shell.rs` (`program_icon`), `crates/huedra-win/src/stores.rs` (`steam_root`), `crates/huedra-ui/src/icons.rs` (worker).
- Each game gets an icon, from the first source that has one: Discord's public list of games (`discord.com/api/v9/applications/detectable`, no account; matched by Steam id, then program path, then exact name; icon from `cdn.discordapp.com` at 64 px), only while online lookups are on; the icon Steam keeps in `appcache\librarycache`; an Xbox game's package logo named in `MicrosoftGame.config`; the icon inside the game's program (Discord's program names as hints, else the largest program whose name matches the game, else the largest; searched level by level, five levels, 20 000 entries at most). PCGamingWiki only has box art, so it is not used for icons.
- The list (about 13 MB) is fetched once per window at most, and only when a game has no current icon. It reveals nothing about the games installed; each icon download names one game's Discord id to Discord's image server.
- Icons are kept as 64 px PNG files in `icons\` (local app-data), named by a hash of the game id, with `icons\index.toml` recording the source; they are looked for again after 30 days, or when online lookups are turned on. Downloads are paced 200 ms apart, at most 256 KB each; images are decoded in Rust (`png`, `zune-jpeg`) with size limits, never by the renderer from the network.
- The General switch is now "Game information and icons"; when it is off, icons come from Steam and the games' own files.

Verified on this PC: online with an empty cache, all 23 games had icons after 7.1 s (20 from Discord, 1 from Steam, 2 from the games' own files); offline with an empty cache, all 23 (20 from Steam's cache, 3 from the games' files, among them Minecraft's package logos and Deadlock's program); a second opening showed every icon within 0.9 s from the cache; 9.9 MB private memory with icons.
Not verified: Epic, GOG, EA, Ubisoft and Rockstar games (none installed here; they use Discord by name, then their program).

Status: done.
