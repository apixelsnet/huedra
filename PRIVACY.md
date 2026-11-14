# Privacy

Last updated: 2026-11-14

Huedra has no accounts, no telemetry and no analytics, and runs no servers. Nothing about you, your PC or what you watch or play is sent to the Huedra project.

## The Huedra program

The program reads the name and path of the program in front, the titles of windows of the video players and browsers it watches, and your game libraries, to decide when to turn HDR on. All of this stays on your PC. Window titles are not written to the log unless you turn that on.

Two features use the internet. Both are on by default, and both can be turned off under Settings:

- **Update checks.** Once a day, the program downloads the release files from the project's GitHub releases. It sends nothing but a `Huedra/<version>` user agent. GitHub's privacy statement applies to these downloads.
- **Game information.** The settings window looks up installed games on PCGamingWiki, sending each game's Steam ID, or its name for other stores, and downloads Discord's public list of games and the icons of your games from it (`discord.com`, `cdn.discordapp.com`). Results are kept on your PC for 30 days. The privacy policies of PCGamingWiki and Discord apply to these requests.

Settings, the log and caches are kept in `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra` (or in a `data` folder beside a portable copy). `huedra uninstall` names them so you can delete them.

## The Huedra browser extension

The extension does not collect, store or transmit any personal data or browsing information. It runs only on YouTube, Netflix, Disney+, Prime Video, HBO Max and Apple TV, reads whether the open video is in HDR or full screen, and adds a short marker to the tab title for the Huedra program on the same PC. It makes no network requests and uses no cookies, storage, analytics or third-party services.

## Changes

Changes to this policy are listed in this file's history on GitHub.
