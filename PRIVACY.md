# Privacy

Last updated: 2026-11-16

Huedra has no accounts, no telemetry and no analytics, and runs no servers. Nothing about you, your PC or what you watch or play is sent to the Huedra project.

## The Huedra program

The program reads the name and path of the program in front, the titles of windows of the video players and browsers it watches, and your game libraries, to decide when to turn HDR on. All of this stays on your PC. Window titles are not written to the log unless you turn that on.

Three features use the internet. All are on by default, and all can be turned off under Settings:

- **Update checks.** Once a day, the program downloads the release files from the project's GitHub releases. It sends nothing but a `Huedra/<version>` user agent. GitHub's privacy statement applies to these downloads.
- **Game information.** The settings window looks up installed games on PCGamingWiki, sending each game's Steam ID, or its name for other stores, and downloads Discord's public list of games and the icons of your games from it (`discord.com`, `cdn.discordapp.com`). Results are kept on your PC for 30 days. The privacy policies of PCGamingWiki and Discord apply to these requests.
- **HDR mods.** Once a day, the settings window downloads the public RenoDX and Luma lists of HDR mods from GitHub (`raw.githubusercontent.com`). Nothing about your games is sent; the lists are matched to your games on your PC. Only when you install a mod does it download that mod from the link in its list (`github.com` or a `github.io` site) and, for RenoDX, ReShade from `reshade.me`. These requests send only a user agent naming Huedra and its version. The privacy policies of GitHub and ReShade apply to them.

Settings, the log and caches are kept in `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra` (or in a `data` folder beside a portable copy). `huedra uninstall` names them so you can delete them.

## The Huedra browser extension

The extension does not collect, store or transmit any personal data or browsing information. It runs only on YouTube, Netflix, Disney+, Prime Video, HBO Max and Apple TV, reads whether the open video is in HDR or full screen, and adds a short marker to the tab title for the Huedra program on the same PC. It makes no network requests and uses no cookies, storage, analytics or third-party services.

## Changes

Changes to this policy are listed in this file's history on GitHub.
