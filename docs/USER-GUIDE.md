# Huedra user guide

This guide covers installing Huedra, the everyday controls, every setting, and what to do when something does not work as expected.

**Contents**

1. [Requirements](#1-requirements)
2. [Install](#2-install)
3. [First start](#3-first-start)
4. [The tray icon and its menu](#4-the-tray-icon-and-its-menu)
5. [Modes](#5-modes)
6. [Games](#6-games)
7. [HDR video](#7-hdr-video)
8. [Displays](#8-displays)
9. [General settings](#9-general-settings)
10. [Updates](#10-updates)
11. [Keyboard shortcuts](#11-keyboard-shortcuts)
12. [Command line](#12-command-line)
13. [Troubleshooting](#13-troubleshooting)
14. [Portable use](#14-portable-use)
15. [Uninstall](#15-uninstall)
16. [Privacy](#16-privacy)

## 1. Requirements

- Windows 10 version 1903 or later, or Windows 11, on a 64-bit (x64) PC.
- At least one display that supports HDR, with HDR available under **Settings › System › Display**.
- About 10 MB of disk space. No other software is needed.

## 2. Install

1. Open the [releases page](../../../releases) and choose the newest release. Releases before 1.0.0 are betas and are marked **Pre-release**.
2. Under **Assets**, download `huedra-<version>-windows-x64.zip`. The other files are optional; the table in the release notes says what each one is.
3. Create a folder of your own for Huedra, for example `%LOCALAPPDATA%\Programs\Huedra`, and unpack the zip into it. Keep the folder writable: updates replace `huedra.exe` there.
4. Start `huedra.exe`.

**Windows SmartScreen.** Huedra is not yet signed with a Windows code-signing certificate, so SmartScreen may show "Windows protected your PC" the first time. Choose **More info**, check that the file is `huedra.exe`, then choose **Run anyway**.

**Verifying the download (optional).** Every release is signed with the project's [minisign](https://jedisct1.github.io/minisign/) key. With minisign installed (`winget install jedisct1.minisign`), run the command shown in the release notes, for example:

```powershell
minisign -Vm huedra-0.1.0-windows-x64.zip -P <public key from the release notes>
```

"Signature and comment signature verified" means the file is exactly the one the project published.

## 3. First start

The first time Huedra starts, its window shows a disclaimer. Huedra changes a Windows display setting, so please read it.

- **Accept and continue** starts Huedra in the notification area.
- **Exit** closes Huedra without changing anything.

Until the disclaimer is accepted, Huedra does not run and never changes HDR. You can read it again at any time under **General › About › Disclaimer**.

After that, Huedra:

- adds itself to Windows start-up (turn this off under **General › Start with Windows**);
- reads the game libraries of the installed stores;
- starts in **Automatic** mode.

Nothing else needs to be set up.

## 4. The tray icon and its menu

Huedra lives in the notification area next to the clock. If you do not see it, open the arrow (^) beside the clock; you can drag the icon onto the taskbar to keep it visible.

| Icon | Meaning |
| --- | --- |
| Coloured gamut shape | HDR is on |
| Grey outline | SDR |

- **Left-click** opens the Huedra window.
- **Right-click** opens the menu:

| Menu item | What it does |
| --- | --- |
| Automatic / Always HDR / Always SDR | Choose the mode (see [Modes](#5-modes)). |
| Pause for 1 hour / Resume automatic switching | Stop automatic switching for an hour, or start it again. |
| Never for *game* | Shown while a game or player is active: Huedra stops changing HDR for it. |
| Huedra *version* is available... | Shown when an update is waiting. Opens the update dialog. |
| Open Huedra | Open the window. |
| Report a problem... | Copy the details for a problem report to the clipboard (see [Troubleshooting](#13-troubleshooting)). |
| Support Huedra on Ko-fi... | Open the project's Ko-fi page in the browser. |
| Exit | Close Huedra. Displays that Huedra switched to HDR return to SDR. |

## 5. Modes

| Mode | Behaviour |
| --- | --- |
| **Automatic** | HDR turns on while a game or HDR video runs and off again when it ends. This is the normal mode. |
| **Always HDR** | HDR stays on for the main display (the one Windows uses as the primary display), or for every managed display when **Displays › Always HDR on the main display only** is off. |
| **Always SDR** | HDR stays off. |

Change the mode from the tray menu, the **Status** page, or with **Win+Alt+O**.

Huedra only ever turns off HDR that it turned on itself. If you switched HDR on in Windows yourself, Huedra leaves it on.

## 6. Games

### Found automatically

Huedra reads the libraries of Steam, Epic Games, GOG, Xbox (Microsoft Store), Ubisoft Connect, EA app and Rockstar Games. Installed games appear on the **Games** page. Choose **Scan again** after installing a game while Huedra is open.

A game that is not listed, for example one installed without a store, can be added with **Add game**: choose its program file.

### Settings for one game

Select a game to open its settings:

| Setting | Meaning |
| --- | --- |
| **Enable while running** | HDR turns on while the game runs. The default. |
| **Do not change** | Huedra leaves HDR alone for this game. |
| **Game controls HDR** | For games with their own HDR switch: Huedra does not interfere. |
| **Delay before enabling HDR** | Waits this many seconds after the game starts. Useful for games that show a launcher first. |
| **Keep HDR on after the game closes** | Seconds HDR stays on after the game closes, so restarting it, or a launcher starting the game again, does not switch the display off and on twice. |

When **Game information and icons** is on, the page also shows what PCGamingWiki reports about the game's HDR support and may suggest a setting. Suggestions are never applied without your choice.

### Start a game with HDR already on

Some games behave badly when HDR changes while they start. For these, HDR can be turned on first:

- **Steam:** the game's page in Huedra shows a launch option. In Steam, right-click the game › **Properties** › **Launch options**, and paste it. It looks like this:
  ```text
  "C:\path\to\huedra.exe" launch -- %command%
  ```
- **Desktop shortcut:** the game's page has **Create a desktop shortcut**, which starts the game with HDR.
- **Any program:** turn on **General › "Start with HDR" in the menu of programs**. Then right-click a program in File Explorer, choose **Show more options**, and choose **Start with HDR**.

HDR stays on until the game and the programs it started have closed.

## 7. HDR video

### Video players

Huedra watches VLC, mpv, mpv.net, MPC-HC, MPC-BE and PotPlayer. On the **Video players** page, choose for each player:

| Choice | Behaviour |
| --- | --- |
| **When HDR video plays** | HDR turns on when the window title shows an HDR file. |
| **While open** | HDR is on whenever the player is open. |
| **Never** | The player is ignored. |

- **Words that mark HDR video** lists the words looked for in the window title, by default `HDR`, `HDR10`, `Dolby Vision`, `DoVi` and `BT.2020`.
- **Only in full screen** turns HDR on only while the player fills the screen.
- **Keep HDR on after the video ends** keeps HDR on for a few seconds after the video ends, so the next episode does not switch the display off and on again.

### Browsers and the Huedra browser extension

Browsers change HDR only through the Huedra browser extension. With it, HDR turns on while an HDR video is open on YouTube and, if you choose, while a video plays in full screen on Netflix, Disney+, Prime Video, HBO Max or Apple TV (**Video players › Streaming sites**).

Until the extension is published in the browser stores, install it by hand:

1. From the release, download `huedra-browser-extension-<version>.zip` and unpack it into a folder you keep.
2. **Chrome, Edge, Brave, Opera, Vivaldi:** open the extensions page (`chrome://extensions` or `edge://extensions`), turn on **Developer mode**, choose **Load unpacked** and select the folder.
3. **Firefox:** open `about:debugging#/runtime/this-firefox`, choose **Load Temporary Add-on**, and select `manifest.json` in the folder. Firefox removes temporary add-ons when it closes.

After HDR turns on, the extension asks YouTube's player to switch to its HDR stream.

### SDR video with RTX Video HDR

With NVIDIA RTX Video HDR turned on in the NVIDIA app, ordinary SDR video in a browser can be shown in HDR. When **SDR video with RTX Video HDR** is on, Huedra turns HDR on while such a video plays in full screen with sound.

## 8. Displays

| Setting | Meaning |
| --- | --- |
| **Managed** | Huedra changes HDR only on managed displays. Displays that support HDR are managed by default. |
| **Only the display showing the content** | HDR turns on only for the display the game or video is on, and follows the window to another display. When off, every managed display switches together. |
| **Always HDR on the main display only** | Always HDR mode and the Win+Alt+H shortcut turn HDR on only for the main display, the one Windows uses as the primary display. Other displays keep following their games and video. When off, every managed display. On by default. |
| **Retry attempts** | How often Huedra tries again when a display does not accept a change. |

## 9. General settings

| Setting | Meaning |
| --- | --- |
| **Start with Windows** | Starts Huedra in the notification area when you sign in. |
| **"Start with HDR" in the menu of programs** | Adds Start with HDR to File Explorer's menu (see [Games](#start-a-game-with-hdr-already-on)). |
| **Updates** | Ask before installing (the default), Install automatically, or Off (see [Updates](#10-updates)). |
| **Game information and icons** | Looks up games on PCGamingWiki and icons in Discord's public game list. |
| **Keep HDR on after a game or video ends** | Seconds HDR stays on after a game closes or a video ends, so restarting the game or playing the next video does not switch the display off and on again. Games and the Video players page can set their own time. |
| **Default delay before enabling HDR** | Seconds to wait after a game starts, unless set per game. |
| **Keyboard shortcuts** | Turns the shortcuts in [Keyboard shortcuts](#11-keyboard-shortcuts) on or off. |
| **Record window titles in the log** | Off by default, so tab names and file names stay out of the log. |

The **About** section shows the version, **Check for updates**, the disclaimer, and links to star the project on GitHub or support it on Ko-fi. Every feature is free; supporters are thanked in `SUPPORTERS.md` and get preview builds before each release.

## 10. Updates

- Huedra checks once a day. When a new version is found, the **Update available** dialog opens, but never while a game or video is using HDR: it waits until they end. It lists what is new; when there are many changes, **See the full changelog** opens the complete list on GitHub.
- **Download and install** installs the update. Huedra restarts in the notification area and the settings window opens again on the new version. **Later** closes the dialog; it opens again the next day, and the tray menu offers the update meanwhile.
- With **Updates** set to **Install automatically**, there is no dialog: Huedra installs new versions on its own while no game or video is using HDR.
- Every update must carry a valid signature from the project's key, and must match the size and SHA-256 digest that the signature covers. Anything else is refused. The list of changes is signed too.
- **Check for updates** under **General › About** checks immediately and opens the dialog when a newer version is found.
- The previous version is kept beside the program as `huedra.previous.exe`. To go back, run `huedra update --rollback` in a terminal opened in Huedra's folder. A version you rolled back from is not installed automatically again.

## 11. Keyboard shortcuts

| Keys | Action |
| --- | --- |
| Win+Alt+H | Turn HDR on or off |
| Win+Alt+O | Change the mode |
| Win+Alt+P | Pause or resume automatic switching |

If another program already uses one of these, the **Keyboard shortcuts** setting says which.

## 12. Command line

Open a terminal in Huedra's folder (in File Explorer, right-click the folder background and choose **Open in Terminal**):

```text
huedra                       Start Huedra in the notification area.
huedra settings [page]       Open the window on a page: status, games, video, displays or general.
huedra status                Show what the running Huedra is doing.
huedra games                 List the games found in every store.
huedra mode automatic|hdr|sdr
huedra pause [minutes]       Pause automatic switching (default 60).
huedra resume                Resume automatic switching.
huedra toggle                Turn HDR on or off, as Win+Alt+H does.
huedra launch -- <program> [arguments]
                             Turn HDR on, start the program, keep HDR on until it ends.
huedra hdr status            List the displays and their HDR state.
huedra hdr on|off [--display N | all]
huedra update [--check]      Install a newer signed release now, or only report it.
huedra update --rollback     Go back to the version before the last update.
huedra uninstall             Close Huedra and remove everything it added to the registry.
huedra exit                  Close the running Huedra.
huedra --version
```

## 13. Troubleshooting

**A display stayed in HDR (or SDR).**
Turn HDR off under **Settings › System › Display**, or run `huedra hdr off --display all`. Huedra also keeps a record of the HDR it turned on and returns those displays to SDR the next time it starts.

**The screen goes black for a moment when HDR changes.**
That is Windows changing the display mode. It takes one to two seconds and happens the same way when you switch HDR in Windows Settings.

**A game does not turn HDR on.**
Check that it is listed on the **Games** page and set to **Enable while running**. If it is missing, choose **Scan again**, or **Add game** and select its program. Some games start through a launcher; a **Delay before enabling HDR** of a few seconds helps.

**HDR turns on for the wrong display.**
Turn on **Displays › Only the display showing the content**, and check that each display is **Managed** as you want.

**A browser does not turn HDR on.**
Browsers need the Huedra browser extension (see [HDR video](#7-hdr-video)). YouTube marks only videos it serves in HDR.

**Is it safe with anti-cheat software?**
Huedra reads only the name and path of the program in front and waits for programs to close. It never reads or writes another program's memory and never injects code. No game publisher has reviewed it, so this cannot be guaranteed.

**Where is the log?**
`%LOCALAPPDATA%\Huedra\huedra.log`, or `data\huedra.log` beside a portable copy. **General › About** shows the folder.

**Huedra closed unexpectedly.**
On the next start Huedra asks whether to copy the details. Choose **Yes**: the details are copied to the clipboard. **Report a problem...** in the tray menu does the same at any time. The details hold the Huedra and Windows versions, the last crash and the last 100 lines of the log, with your user folder shown as `%USERPROFILE%`. Huedra sends nothing itself; read them, then paste them into your problem report.

**Still stuck?**
Open an issue on the project's GitHub page and paste the details from **Report a problem...** in the tray menu, or attach the relevant part of the log. Report security problems privately, as described in [SECURITY.md](../SECURITY.md).

## 14. Portable use

Create an empty file named `portable` next to `huedra.exe`. Huedra then keeps its settings, log and caches in a `data` folder beside it, and does not add itself to Windows start-up.

## 15. Uninstall

1. Run `huedra uninstall` in Huedra's folder. It closes Huedra and removes the start-up entry and the "Start with HDR" menu entry.
2. Delete Huedra's folder.
3. To remove the settings, the log and the caches as well, delete `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra`.

Desktop shortcuts created by Huedra are ordinary files and can be deleted like any other.

## 16. Privacy

Huedra has no accounts and no telemetry. Only the update check and the game information lookups use the internet, and both can be turned off. The full statement, including the browser extension, is in [PRIVACY.md](../PRIVACY.md).
