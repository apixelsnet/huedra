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

## Done when
Every command works against a running tray process, and a second tray process exits with a message.

Status: phase 0 stub only.
