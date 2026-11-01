# 10 — Configuration

Where settings and logs live.

## Scope
- `config.toml` in the roaming app-data folder (`Huedra`); portable mode when a file named `portable` sits next to the exe.
- Size-limited before parsing; written to a temporary file, then renamed.
- Versioned, with upgrades between versions.
- Log in the local app-data folder (`Huedra`), 1 MB x 2 files. Window titles only when the user enables it.

## Done when
A config from every earlier version loads, and a power cut during a write never leaves a broken file.

## Result (2026-10-31) — phase 2
Code: `crates/huedra-core/src/{config,files,logfile,clock}.rs`, `crates/huedra/src/paths.rs`.
- Schema version 1: `mode`, `[general]` (`start_with_windows`, `updates`, `game_info`, `start_delay_secs`, `hold_secs`, `retry_attempts`, `log_window_titles`, `hotkeys`), `[displays] unmanaged`, `content_only` (default true: HDR only on the display showing the game or video), `manual` (`"main"` by default: Always HDR mode and the shortcut switch only the main display; `"all"`: every managed display), `[[game]]` (`id`, `name`, `exe`, `hdr`, `start_delay_secs`, `hold_secs`, `launch_with_hdr`). Unknown keys are ignored; a newer schema is refused rather than overwritten; values are clamped (delay 120 s, hold 600 s, retries 5). A leading byte-order mark is accepted.
- Files over 1 MB are refused before reading; writes go to `<name>.tmp`, are flushed, then renamed over the old file.
- Portable mode: a file named `portable` next to the exe keeps everything in `data\` beside it.
- Log `huedra.log`, 1 MB, rotated to `huedra.log.1`; one line per entry. Window titles are not logged at all yet.
- The mode is saved when changed, so it survives restarts.

Verified on this PC: portable mode in `dist\`; a config written by Windows PowerShell 5.1 (with byte-order mark) loads.
Not verified: a power cut during a write.

## Result (2026-10-31) — phase 3
- `huedra_core::locations::Locations` replaces the executable's own path code, so the tray, the settings window and `huedra games` share one list: `config.toml` (roaming), `owned.toml`, `huedra.log`, `gameinfo.toml`, `window.toml` (local).
- The settings window saves every change at once and asks the tray process to reload.


## Result (2026-10-31) — end-to-end pass
- Both writers of `config.toml`, the tray and the settings window, now read the file again just before changing it and write only their change back (`edit_config` in `crates/huedra/src/app.rs`, `edit` in `crates/huedra-ui/src/app.rs`). A copy loaded earlier used to overwrite the other process's newer change; the end-to-end test caught the settings window restoring an old mode.

Verified on this PC: mode changed from the command line while the settings window was open, then a setting changed in the window: both changes kept.

Status: done.

## Result (2026-10-31) — phase 4
- `[video]`: `keywords`, `full_screen_only` (default true), `sdr_with_rtx` (default true), `sites` (streaming sites where full-screen video counts as HDR: `netflix`, `disney-plus`, `prime-video`, `hbo-max`, `apple-tv`; none by default), `hold_secs` (optional, clamped to 600), and `[video.apps]` mapping a program key (`vlc`, `mpv`, `mpv-net`, `mpc-hc`, `mpc-be`, `pot-player`, `chrome`, `edge`, `firefox`, `brave`, `opera`, `vivaldi`) to `when-hdr-plays`, `while-open` or `never`. Still schema version 1: older files get the defaults.

## Result (2026-10-31) — Video players page
- The settings window writes `[video.apps]` only for programs not set to `when-hdr-plays`, and an empty keyword line restores the default words (`config::default_keywords`). The tray's "Never for" uses the same `VideoSettings::set_mode`.

## Result (2026-11-01) — phase 5
- `update.toml` in the local folder (or `data` when portable): `last_check` (seconds since 1970), `available` (a version waiting for consent), `skipped` (a version rolled back from). Missing or damaged means "never checked". `[general] updates` = `install` (default), `ask` or `off`.

## Result (2026-11-01) — disclaimer flag
- `[general] disclaimer_accepted` (default `false`): set by the settings window's Accept and continue. Until it is `true` the tray does not run. Existing settings files without it show the disclaimer once.
