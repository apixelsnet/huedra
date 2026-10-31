# 10 — Configuration

Where settings and logs live.

## Scope
- `config.toml` in the roaming app-data folder (`Huedra`); portable mode when a file named `portable` sits next to the exe.
- Size-limited before parsing; written to a temporary file, then renamed.
- Versioned, with upgrades between versions.
- Log in the local app-data folder (`Huedra`), 1 MB x 2 files. Window titles only when the user enables it.

## Done when
A config from every earlier version loads, and a power cut during a write never leaves a broken file.

Status: not started (phase 2).
