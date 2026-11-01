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

## Result (2026-10-31) — phase 4
- Title and move hooks are per process (`SetWinEventHook` with a process id), installed only for known video programs; see `05-video.md`. Repeated window-change events for one process are queued once.

## Result (2026-10-31) — Game window display
Code: `find_game_window`, `place_game`, `game_target`, `retarget_all` in `crates/huedra/src/app.rs`; `main_monitor` in `crates/huedra-win/src/window.rs`; `Tracker::trigger_of`.
- With Displays, "Only the display showing the content" on (the default), a game turns HDR on only for the display of its largest visible window; until it has a window, nothing switches (owner's choice: games that read HDR state only at startup may miss it). The display is read again when the game comes to the front and on its window moves (a per-process hook), and kept while the window is minimised. A launcher's successor takes over its display when its window appears.
- Changing the setting applies to running games and video at once.

Verified: build green (clippy for Windows with `-D warnings`).
Not verified on this PC: any game; the owner tests it.

## Result (2026-11-01) — first real game
Verified by the owner on this PC (build cf4b7b9, Automatic mode, keep HDR on 3 s, "Only the display showing the content" on): Resident Evil 4 started from Steam without the launch option turned HDR on for Display 1, the display its window was on, in the same second ("Switch requested 3 ms after the event"); the log shows no switch on Display 2. Quitting the game returned Display 1 to SDR 3 s later, as the hold time says.

Not verified: whether the game itself offered its HDR option; Alt+Tab while the game runs; the Steam launch option (see 06).

Status: done for games; video titles in progress (phase 4).
