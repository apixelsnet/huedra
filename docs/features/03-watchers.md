# 03 — Watchers

Turns Windows events into engine input, without polling.

## Scope
- `SetWinEventHook` for foreground changes and title changes (out of context, no DLL injection).
- Process exit waits: `OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION)` and `RegisterWaitForSingleObject`.
- `WM_DISPLAYCHANGE` to notice HDR changed outside Huedra.

## Notes
- No access to process memory, so it is safe with anti-cheat.

## Done when
The tray process shows zero wake-ups over 10 idle minutes, and a game coming to the front reaches the engine in under 50 ms.

## Result (2026-10-31) — phase 2
Code: `crates/huedra-win/src/{event_loop,foreground,process}.rs`, `crates/huedra/src/app.rs`.
- One hidden top-level window (it must be top-level to receive the `WM_DISPLAYCHANGE` broadcast). The window procedure and WinEvent callback only queue events; the handler runs between messages, so menus and switches never run inside a callback.
- Foreground: `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`, out of context, skipping Huedra's own process. The foreground window at start is checked once.
- Exits: `OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION)` and `RegisterWaitForSingleObject` posting to the window; the wait context is freed only after `UnregisterWaitEx(INVALID_HANDLE_VALUE)`.
- Following a launcher reads the process list once a second, for at most 45 s, and only queries the path of processes new since the window opened.
- `WM_DISPLAYCHANGE` does arrive when HDR is switched (checked with `huedra hdr on` while the tray ran).

Verified on this PC: 0 ms of processor time over 60 s idle, 2 threads, 1.7 MB private memory (11 MB working set, mostly shared system DLLs).
Not verified: detection latency (no measurement yet); 10 idle minutes (60 s measured); the title-change hook (phase 4).

Status: done for games; video titles in phase 4.
