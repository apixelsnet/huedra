# 11 — Updates

## Scope
- Default "Install automatically"; also "Ask before installing" and "Off" (Settings, General page).
- Check once a day; install only while no game or video is running; keep the previous version for a one-step rollback.
- Verify the minisign Ed25519 signature against the key built into the exe, then SHA-256, then replace by rename.
- WinHTTP; no TLS crate.

## Notes
- Hosting is deferred. The release address is set when it is decided; until then the feature is inactive.

## Done when
A signed test release installs, a tampered one is refused with a clear message, and rollback restores the previous exe.

Status: not started (phase 5).
