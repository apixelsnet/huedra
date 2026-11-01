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

## Result (2026-10-31) — open items
- The tray and settings roles install a panic hook that writes the panic to `huedra.log` before the process ends; release builds abort on panic and neither role has a console, so a closed window or a vanished tray icon used to leave no trace. The tray's recovery record still restores SDR on the next start.

Verified on this PC: a forced panic in the settings window (a window resized by a program to 850 x 65 000 pixels) wrote "Huedra stopped after an internal error: panicked at ... euclid ... size.rs" to the log.


## Result (2026-10-31) — end-to-end pass
- `huedra exit` replies only after the displays are back in SDR, so "Huedra has closed." is true when printed and a script can rely on it; before, the reply came first and the switch about a second later.
- The log names a game when it ends ("Character Map ended.") as it does when it starts; it used to print the internal id. `Engine::label` gives the name; unit test `label_names_a_running_trigger_only`.
- The tray saves its own changes (the mode, "never for this game") as a read-modify-write of `config.toml`, so a change saved by the settings window and not yet reloaded is kept.

Verified on this PC, 24 end-to-end checks against the release build: version, help and usage errors; `games`; status with and without the tray; a second start opening the settings window; Always HDR, Automatic with its hold, a real program set up as a game (on in about 2 s, held 3 s after it closed, then SDR), pause and resume, toggle on and off, SDR at the moment `exit` returns, the saved mode applied at the next start.

## Result (2026-10-31) — phase 4
- `launch` and `uninstall` added (see 06). `Request::Launch { pid, program }` is answered once HDR is on, or with the reason it stays off. `launch` passes the started program's exit code on.

Status: done.

## Result (2026-11-01) — phase 5
- `huedra update [--check] [--from <folder>]`, `update --rollback`, and the tray's internal `update --auto` (see 11). `Request::UpdateAvailable { version }` puts a waiting release in the tray menu; `StatusReport::busy` (default false when absent) tells the updater that a game, video or launch uses HDR. Protocol version unchanged: the new field is optional and the new request is only sent to a tray of the same build.
- `pipe::wait_closed` waits until no tray owns the pipe (`WaitNamedPipeW` reports the name missing), without connecting.

Verified on this PC: unit tests for the command forms; `update --from` and `--rollback` closed and restarted a test tray through the pipe.

## Result (2026-11-01) — `--background`
- `huedra` opens the window as well as the tray; `huedra --background` is the tray alone (see 07). The help text lists both.

Verified: unit tests (`--background`, and `--background` with an extra argument refused).
