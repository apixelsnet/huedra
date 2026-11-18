# Security

## Supported versions

Only the latest release receives fixes. Huedra checks for updates once a day and, by default, asks before installing one, so please install updates when they are offered. Install automatically, under Settings › Updates, applies them without asking.

## Reporting a problem

Please report security problems privately, through the repository's **Security** tab ("Report a vulnerability"), not in a public issue. Include the Huedra version (`huedra --version`), the Windows version, and the steps that show the problem. You should receive an answer within a week.

## What Huedra does and does not do

- Runs with the user's normal rights and never asks for administrator rights.
- Reads the name and path of programs that come to the front, and window titles of known video players and browsers. It never reads or writes another program's memory and never places code in another program. The one exception is an HDR mod you choose to install: its files (ReShade and the mod) are copied into the game's folder, and the game loads them itself when it starts.
- Writes only to `%APPDATA%\Huedra`, `%LOCALAPPDATA%\Huedra` (or the `data` folder of a portable copy), its own program folder during an update, desktop shortcuts you ask it to create, and a game's folder only when you install or remove an HDR mod there (every file it writes is recorded with its SHA-256, and files it replaces are backed up; removing a mod you installed yourself deletes only the files the dialog lists, after copying them to `mods\removed` in Huedra's local folder). In the registry, under `HKEY_CURRENT_USER`: the start-up value `Software\Microsoft\Windows\CurrentVersion\Run\Huedra`, the "Start with HDR" entry `Software\Classes\exefile\shell\Huedra.StartWithHdr`, and the Auto HDR pair of game programs under `Software\Microsoft\DirectX\UserGpuPreferences`. It also sets, for the current user, the color profiles Windows uses for a display while a game with its own profiles runs, and, on NVIDIA graphics cards, the RTX HDR setting in game profiles of the NVIDIA driver. `huedra uninstall` removes the start-up value and the menu entry, removes the HDR mods it installed, and puts back the Auto HDR and RTX HDR values it changed.
- The settings window draws the Status card's banner in HDR with Windows' own Direct3D 11 and DirectComposition (`d3d11.dll`, `dxgi.dll`, `dcomp.dll`) on Windows 11 22H2 and later: a surface of the window itself, no other process involved. Starting the window with the environment variable `HUEDRA_HDR_LAYER=0` turns it off.
- Talks to its other parts through a named pipe that only the current Windows user can open and that refuses connections from other computers. Every message is versioned, size-limited and checked.
- Connects to the internet only for update checks and, in the settings window, for PCGamingWiki, Discord's public list of games, and the RenoDX and Luma lists of HDR mods on GitHub. All can be turned off. When you install an HDR mod, it downloads the mod only from `github.com`, `*.github.io` or `reshade.me` over HTTPS, at most 1 GB, and unpacks it with Windows' own `tar.exe` into a work folder before copying anything into the game's folder.
- Installs an update only when its description is signed with the project's minisign key (Ed25519), built into the program, and the downloaded program matches the signed size and SHA-256 digest. Older versions are never installed. The old program is replaced: the new version deletes it on its first start.
- The browser extension runs only on the sites it lists, stores nothing, makes no network requests of its own, and signals Huedra only through the tab title. No local network port is opened.

## Known limitations

- The program is not yet signed with a Windows code-signing certificate, so SmartScreen warns on first start.
- Anyone who can write to Huedra's program folder can replace the program; install it in a folder only your account can write to.
