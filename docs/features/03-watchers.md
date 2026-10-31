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

Status: not started (phase 2).
