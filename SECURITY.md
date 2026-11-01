# Security

## Supported versions

Only the latest release receives fixes. Huedra checks for updates once a day and, by default, asks before installing one, so please install updates when they are offered. Install automatically, under General › Updates, applies them without asking.

## Reporting a problem

Please report security problems privately, through the repository's **Security** tab ("Report a vulnerability"), not in a public issue. Include the Huedra version (`huedra --version`), the Windows version, and the steps that show the problem. You should receive an answer within a week.

## What Huedra does and does not do

- Runs with the user's normal rights and never asks for administrator rights.
- Reads the name and path of programs that come to the front, and window titles of known video players and browsers. It never reads or writes another program's memory and never places code in another program.
- Writes only to `%APPDATA%\Huedra`, `%LOCALAPPDATA%\Huedra` (or the `data` folder of a portable copy), its own program folder during an update, and these registry values under `HKEY_CURRENT_USER`: the start-up value `Software\Microsoft\Windows\CurrentVersion\Run\Huedra` and the "Start with HDR" entry `Software\Classes\exefile\shell\Huedra.StartWithHdr`. `huedra uninstall` removes both.
- Talks to its other parts through a named pipe that only the current Windows user can open and that refuses connections from other computers. Every message is versioned, size-limited and checked.
- Connects to the internet only for update checks and, in the settings window, for PCGamingWiki and Discord's public list of games. Both can be turned off.
- Installs an update only when its description is signed with the project's minisign key (Ed25519), built into the program, and the downloaded program matches the signed size and SHA-256 digest. Older versions are never installed automatically, and the previous version is kept for `huedra update --rollback`.
- The browser extension runs only on the sites it lists, stores nothing, makes no network requests of its own, and signals Huedra only through the tab title. No local network port is opened.

## Known limitations

- The program is not yet signed with a Windows code-signing certificate, so SmartScreen warns on first start.
- Anyone who can write to Huedra's program folder can replace the program; install it in a folder only your account can write to.
