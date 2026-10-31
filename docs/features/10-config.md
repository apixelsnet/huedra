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
- Schema version 1: `mode`, `[general]` (`start_with_windows`, `updates`, `game_info`, `start_delay_secs`, `hold_secs`, `retry_attempts`, `log_window_titles`, `hotkeys`), `[displays] unmanaged`, `[[game]]` (`id`, `name`, `exe`, `hdr`, `start_delay_secs`, `hold_secs`, `launch_with_hdr`). Unknown keys are ignored; a newer schema is refused rather than overwritten; values are clamped (delay 120 s, hold 600 s, retries 5). A leading byte-order mark is accepted.
- Files over 1 MB are refused before reading; writes go to `<name>.tmp`, are flushed, then renamed over the old file.
- Portable mode: a file named `portable` next to the exe keeps everything in `data\` beside it.
- Log `huedra.log`, 1 MB, rotated to `huedra.log.1`; one line per entry. Window titles are not logged at all yet.
- The mode is saved when changed, so it survives restarts.

Verified on this PC: portable mode in `dist\`; a config written by Windows PowerShell 5.1 (with byte-order mark) loads.
Not verified: a power cut during a write.

## Result (2026-10-31) — phase 3
- `huedra_core::locations::Locations` replaces the executable's own path code, so the tray, the settings window and `huedra games` share one list: `config.toml` (roaming), `owned.toml`, `huedra.log`, `gameinfo.toml`, `window.toml` (local).
- The settings window saves every change at once and asks the tray process to reload.

Status: done.
