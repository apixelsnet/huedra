# Huedra user guide

This guide covers installing Huedra, the everyday controls, every setting, and what to do when something does not work as expected. For a step-by-step walkthrough of the first setup, the browser extension and troubleshooting, see the [setup guide](SETUP.md).

**Contents**

1. [Requirements](#1-requirements)
2. [Install](#2-install)
3. [First start](#3-first-start)
4. [The tray icon and its menu](#4-the-tray-icon-and-its-menu)
5. [Modes](#5-modes)
6. [Games](#6-games)
7. [HDR video](#7-hdr-video)
8. [Displays](#8-displays)
9. [Settings](#9-settings)
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

1. Open the [releases page](../../../releases) and choose the newest release, marked **Latest**. Releases before 1.0.0 were betas and are marked **Pre-release**.
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

- **Accept and continue** starts Huedra; the window stays open.
- **Exit** closes Huedra without changing anything.

After you accept it the first time, a short setup asks which displays Huedra may control, whether to look up your games' HDR support on PCGamingWiki, and which mode to use; **Skip setup** keeps the defaults. Until the disclaimer is accepted, Huedra does not run and never changes HDR. You can read it again at any time under **Settings › About › Disclaimer**.

After that, Huedra:

- adds itself to Windows start-up (turn this off under **Settings › Start with Windows**);
- reads the game libraries of the installed stores;
- starts in **Automatic** mode.

Nothing else needs to be set up.

Whenever you start `huedra.exe` yourself, the window opens. When Huedra is running already, starting it again brings its window to the front. When Windows starts Huedra at sign-in, it stays in the notification area without opening the window.

## 4. The tray icon and its menu

Huedra lives in the notification area next to the clock. If you do not see it, open the arrow (^) beside the clock; you can drag the icon onto the taskbar to keep it visible.

| Icon | Meaning |
| --- | --- |
| Colored gamut shape | HDR is on |
| Gray outline | SDR |

- **Left-click** opens the quick panel beside the icon: whether HDR is on and why, the mode, each display with a switch that turns its HDR on or off now (as Win+Alt+H; the next game or video takes over again), **Pause 1 hour**, **Never for** the game running, and **Open Huedra**. It closes when you click elsewhere, press Escape, or click the icon again.
- **Right-click** opens the menu:

| Menu item | What it does |
| --- | --- |
| Automatic / Always HDR / Always SDR | Choose the mode (see [Modes](#5-modes)). |
| Pause for 1 hour / Resume automatic switching | Stop automatic switching for an hour, or start it again. |
| Never for *game* | Shown while a game or player is active: Huedra stops changing HDR for it. |
| Huedra *version* is available... | Shown when an update is waiting. Opens the update dialog. |
| Open Huedra | Open the window. |
| Report a problem... | Copy the details for a problem report to the clipboard (see [Troubleshooting](#13-troubleshooting)). |
| Support Huedra on Buy Me a Coffee... | Open the project's Buy Me a Coffee page in the browser. |
| Exit | Close Huedra. Displays that Huedra switched to HDR return to SDR. |

## 5. Modes

| Mode | Behavior |
| --- | --- |
| **Automatic** | HDR turns on while a game or HDR video runs and off again when it ends. This is the normal mode. |
| **Always HDR** | HDR stays on for the main display (the one Windows uses as the primary display), or for every display Huedra controls when **Displays › Always HDR on the main display only** is off. |
| **Always SDR** | HDR stays off. |

Change the mode from the tray menu, the **Status** page, or with **Win+Alt+O**.

Huedra only ever turns off HDR that it turned on itself. If you switched HDR on in Windows yourself, Huedra leaves it on.

## 6. Games

### Found automatically

Huedra reads the libraries of Steam, Epic Games, GOG, Xbox (Microsoft Store), Ubisoft Connect, EA app, Rockstar Games, Battle.net, Riot Games, HoYoPlay, Amazon Games, itch.io and Heroic Games Launcher (its Epic, GOG and Amazon games). Installed games appear on the **Games** page. Choose **Scan again** after installing a game while Huedra is open.

Games that no launcher lists but that the Xbox Game Bar recognized as games while they ran are listed too, under **Game Bar**, named after their folder. A program the Game Bar took for a game by mistake can be set to **Do not change**.

For League of Legends, only the game itself counts, not the League client that stays open between matches. Test servers (Riot's PBE, Battle.net's test realms and betas) are not listed.

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

Some games behave badly when HDR changes while they start, and some check HDR only while they start, so their HDR settings stay grayed out when HDR came on later. For these, HDR can be turned on first:

- **Steam, recommended:** **Turn HDR on when Steam starts the game** is on by default for Steam games; switch it off on the game's page if you prefer HDR to wait for the game's window. HDR turns on as soon as Steam reports the game running, about a second after you choose **Play** and before the game's window shows, on the main display until that window appears. Huedra only reads which game Steam reports as running, so this suits games with anti-cheat too.
- **Steam launch option:** the game's page in Huedra also shows a launch option. In Steam, right-click the game › **Properties** › **Launch options**, and paste it. It looks like this:
  ```text
  "C:\path\to\huedra.exe" launch -- %command%
  ```
  Games with anti-cheat (Battlefield 6, for example) may refuse to start this way; use the switch above instead.
- **Desktop shortcut:** the game's page has **Create a desktop shortcut**, which starts the game with HDR.
- **Any program:** turn on **Settings › Advanced › "Start with HDR" in the menu of programs**. Then right-click a program in File Explorer, choose **Show more options**, and choose **Start with HDR**.

HDR stays on until the game and the programs it started have closed.

## 7. HDR video

### Video players

Huedra watches VLC, mpv, mpv.net, MPC-HC, MPC-BE and PotPlayer. On the **Video players** page, choose for each player:

| Choice | Behavior |
| --- | --- |
| **When HDR video plays** | HDR turns on when the window title shows an HDR file. |
| **While open** | HDR is on whenever the player is open. |
| **Never** | The player is ignored. |

- **Words that mark HDR video** lists the words looked for in the window title, by default `HDR`, `HDR10`, `Dolby Vision`, `DoVi` and `BT.2020`.
- **Only in full screen** turns HDR on only while the player fills the screen.
- **Keep HDR on after the video ends** keeps HDR on for a few seconds after the video ends, so the next episode does not switch the display off and on again.

### Browsers and the Huedra browser extension

Browsers change HDR only through the Huedra browser extension. With it, HDR turns on while an HDR video is open on YouTube and, if you choose, while a video plays in full screen on Netflix, Disney+, Prime Video, HBO Max or Apple TV (**Video players › Streaming sites**).

Install it:

- **Chrome, Edge, Brave, Opera, Vivaldi:** install [Huedra from the Chrome Web Store](https://chromewebstore.google.com/detail/huedra/kilkdlcbhddpigcdffdnmdknjhlddlod) and choose **Add to Chrome** (or the button named after your browser). **Get the extension** on the **Video players** page, under Browsers, opens the store page. Edge first asks to allow extensions from other stores; Opera first needs its **Install Chrome Extensions** add-on. The browser keeps the extension up to date.
- **Firefox:** from the release, download `huedra-browser-extension-<version>.zip` and unpack it into a folder you keep. Open `about:debugging#/runtime/this-firefox`, choose **Load Temporary Add-on**, and select `manifest.json` in the folder. Firefox removes temporary add-ons when it closes.

The [setup guide](SETUP.md#7-set-up-the-browser-extension) has the steps in detail, including installing by hand without the store.

After HDR turns on, the extension asks YouTube's player to switch to its HDR stream.

### SDR video with RTX Video HDR

With NVIDIA RTX Video HDR turned on in the NVIDIA app, ordinary SDR video in a browser can be shown in HDR. When **SDR video with RTX Video HDR** is on, Huedra turns HDR on while such a video plays in full screen with sound.

## 8. Displays

| Setting | Meaning |
| --- | --- |
| **Let Huedra control HDR** | One switch per display. Huedra changes HDR only on the displays it controls, in every mode; on the others HDR stays as you set it in Windows. Displays that support HDR are controlled by default. A display without HDR shows **Not available**. |
| **Only the display showing the content** | HDR turns on only for the display the game or video is on, and follows the window to another display. When off, every display Huedra controls switches together. |
| **Always HDR on the main display only** | Always HDR mode and the Win+Alt+H shortcut turn HDR on only for the main display, the one Windows uses as the primary display. Other displays keep following their games and video. When off, every display Huedra controls. On by default. |
| **Retry attempts** | How often Huedra tries again when a display does not accept a change. |

## 9. Settings

The **Settings** page sits at the foot of the navigation pane. It was called **General** before; `huedra settings general` still opens it. The settings people change most are rows of their own; the rest fold into sections whose second line sums up their values. Click a section, or press Space or Enter on it, to open it.

| Setting | Meaning |
| --- | --- |
| **Start with Windows** | Starts Huedra in the notification area when you sign in, without opening the window. |
| **Closing this window** | **Keep Huedra running** (the default): closing the window leaves Huedra switching HDR from the notification area. **Quit Huedra**: closing the window closes Huedra too, and displays it turned to HDR go back to SDR. **Exit** in the tray menu always quits. |
| **Notify me when HDR does not switch** | A Windows notification when a display did not switch after Huedra's retries; normal switches never notify. On by default. |
| **Updates** | Ask before installing (the default), Install automatically, or Off (see [Updates](#10-updates)). |
| **Timing › Keep HDR on after a game or video ends** | Seconds HDR stays on after a game closes or a video ends, so restarting the game or playing the next video does not switch the display off and on again. Games and the Video players page can set their own time. |
| **Timing › Default delay before enabling HDR** | Seconds to wait after a game starts, unless set per game. |
| **Keyboard shortcuts** | The switch in its header turns the shortcuts in [Keyboard shortcuts](#11-keyboard-shortcuts) on or off. |
| **Advanced › "Start with HDR" in the menu of programs** | Adds Start with HDR to File Explorer's menu (see [Games](#start-a-game-with-hdr-already-on)). |
| **Advanced › Record window titles in the log** | Off by default, so tab names and file names stay out of the log. |
| **Advanced › Game information and icons** | Looks up games on PCGamingWiki and icons in Discord's public game list. |

The **About** section (headed with Huedra's version) has **Check for updates**, the disclaimer, the settings folder, and links to star the project on GitHub or support it on Buy Me a Coffee. Every feature is free; supporters are thanked in `SUPPORTERS.md` and get preview builds before each release.

## 10. Updates

- Huedra checks once a day. When a new version is found, the **Update available** dialog opens, but never while a game or video is using HDR: it waits until they end. It lists what is new; when there are many changes, **See the full changelog** opens the complete list on GitHub.
- **Download and install** installs the update. Huedra restarts in the notification area and the settings window opens again on the new version. **Later** closes the dialog; it opens again the next day, and the tray menu offers the update meanwhile.
- With **Updates** set to **Install automatically**, there is no dialog: Huedra installs new versions on its own while no game or video is using HDR.
- Every update must carry a valid signature from the project's key, and must match the size and SHA-256 digest that the signature covers. Anything else is refused. The list of changes is signed too.
- **Check for updates** under **Settings › About** checks immediately and opens the dialog when a newer version is found.
- An update replaces only `huedra.exe`. Your settings, games and history are kept: they live in `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra` (or the `data` folder of a portable copy), which updates never touch. During the update the old program is set aside as `huedra.previous.exe`; the new version deletes it when it starts.

## 11. Keyboard shortcuts

| Keys | Action |
| --- | --- |
| Win+Alt+H | Turn HDR on or off |
| Win+Alt+O | Change the mode |
| Win+Alt+P | Pause or resume automatic switching |

If another program already uses one of these, Huedra's log says which (see [Troubleshooting](#13-troubleshooting)).

## 12. Command line

Open a terminal in Huedra's folder (in File Explorer, right-click the folder background and choose **Open in Terminal**):

```text
huedra                       Start Huedra and open its window (or bring it to the front).
huedra --background          Start Huedra in the notification area only, as at sign-in.
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
Turn on **Displays › Only the display showing the content**, and check **Let Huedra control HDR** for each display as you want.

**A browser does not turn HDR on.**
Browsers need the Huedra browser extension (see [HDR video](#7-hdr-video)). YouTube marks only videos it serves in HDR.

**Is it safe with anti-cheat software?**
Huedra reads only the name and path of the program in front and waits for programs to close. It never reads or writes another program's memory and never injects code. No game publisher has reviewed it, so this cannot be guaranteed.

**Where is the log?**
`%LOCALAPPDATA%\Huedra\huedra.log`, or `data\huedra.log` beside a portable copy. **Settings › About** shows the folder.

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
