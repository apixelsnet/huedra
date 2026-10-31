# 09 — Command line and IPC

One executable, three roles, one channel between them.

## Scope
- Roles: tray process (no arguments), settings window (`settings`), commands (`hdr`, `mode`, `pause`, `launch`, `exit`, `uninstall`, `--version`).
- Single instance per user.
- Named pipe named after the user SID: DACL for that SID only, `PIPE_REJECT_REMOTE_CLIENTS`, versioned and size-limited messages, strict parsing.

## Notes
- Message types live in `huedra-core` and are tested on Linux.

## Result (2026-10-31) — phase 0
Code: `crates/huedra/src/main.rs`
- `huedra` and `huedra --version` print the name and version; any other argument exits with code 2.

## Result (2026-10-31) — phase 1
Code: `crates/huedra-core/src/command.rs` (grammar and tests), `crates/huedra/src/main.rs`, `crates/huedra/build.rs`, `crates/huedra/huedra.manifest`.
- Commands parsed: no arguments (tray), `settings`, `hdr status`, `hdr on|off [--display N | all]`, `--version`, `--help`. Exit codes: 0 success, 1 failure, 2 usage error (with the usage text).
- The manifest is embedded at link time (`/MANIFESTINPUT`): `asInvoker`, per-monitor DPI v2, UTF-8 code page, long paths, and `consoleAllocationPolicy=detached`. The exe stays a console program, so shells wait for it and show its output, while Windows 11 24H2 and later open no console window when it starts from Explorer.

Verified on this PC: usage errors exit 2, a missing display exits 1 with a clear message.

## Done when
Every command works against a running tray process, and a second tray process exits with a message.

## Result (2026-10-31) — phase 2
Code: `crates/huedra-core/src/ipc.rs`, `crates/huedra-win/src/pipe.rs`, `crates/huedra/src/{client,app}.rs`.
- Pipe `\\.\pipe\Huedra-<user SID>`, DACL `D:P(A;;GA;;;<SID>)`, `PIPE_REJECT_REMOTE_CLIENTS`, one instance, created with `FILE_FLAG_FIRST_PIPE_INSTANCE`. A second tray process gets "busy" or "denied", prints "Huedra is already running." and opens the settings window.
- Clients connect with `SECURITY_IDENTIFICATION` and check that the server process runs as the same user (`GetNamedPipeServerProcessId`, token SID comparison).
- Messages: one JSON line each way, `{"v":1,"body":{...}}`, at most 64 KB, version checked before parsing the body.
- New commands: `status`, `mode automatic|hdr|sdr`, `pause [minutes]` (1 to 1440, default 60), `resume`, `toggle`, `exit`.

Verified on this PC: every command against a running tray; a second tray process exits with the message.

## Result (2026-10-31) — phase 3
- `huedra settings [page]` opens the window (see 08); `huedra games` lists the library (see 04).
- The status report also carries the reason line and the ids of the running games, which the Status and Games pages show.
- The tray process opens the window with `CREATE_NO_WINDOW`, so no console appears on any Windows version.

Status: done except `launch` and `uninstall` (phases 4 and 5).
