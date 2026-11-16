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
- About 15 MB of disk space. No other software is needed.
- For NVIDIA RTX HDR only: an NVIDIA graphics card and driver. Everything else works with any graphics card.

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

If you used Huedra before its new window, a notice on **Status** after the update points out the quick panel on the tray icon and Settings at the foot of the menu. **Show setup** opens the same setup screens with your current settings; closing the notice hides it for good.

After that, Huedra:

- adds itself to Windows start-up (turn this off under **Settings › Start with Windows**);
- reads the game libraries of the installed stores;
- starts in **Automatic** mode.

Nothing else needs to be set up.

Whenever you start `huedra.exe` yourself, the window opens. When Huedra is running already, starting it again brings its window to the front. When Windows starts Huedra at sign-in, it stays in the tray without opening the window.

## 4. The tray icon and its menu

Huedra lives in the tray (the notification area, next to the clock). If you do not see it, open the arrow (^) beside the clock; you can drag the icon onto the taskbar to keep it visible.

| Icon | Meaning |
| --- | --- |
| Colored gamut shape | HDR is on |
| Gray outline | HDR is off |

- **Left-click** opens the quick panel beside the icon: whether HDR is on and why, the mode, each display with a switch that turns its HDR on or off now (as Win+Alt+H; the next game or video takes over again), **Pause 1 hour**, **Never use HDR for** the game running, and **Open Huedra**. It closes when you click elsewhere, press Escape, or click the icon again.
- **Right-click** opens the menu:

| Menu item | What it does |
| --- | --- |
| Automatic / Always HDR / Always SDR | Choose the mode (see [Modes](#5-modes)). |
| Pause automatic switching › 15 minutes to 4 hours / Resume automatic switching | Stop automatic switching for a while, or start it again. |
| Never use HDR for *game* | Shown while a game or player is active: Huedra stops changing HDR for it. |
| Install Huedra *version*... | Shown when an update is waiting. Opens the update dialog. |
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

In Automatic and Always HDR, Huedra only turns off HDR that it turned on itself: if you switched HDR on in Windows yourself, Huedra leaves it on. Choosing **Always SDR**, or starting Huedra in it, turns HDR off on every display Huedra controls, whoever turned it on. If you then switch HDR on in Windows while Always SDR stays selected, Huedra leaves it on.

## 6. Games

### Found automatically

Huedra reads the libraries of Steam, Epic Games, GOG, Xbox (Microsoft Store), Ubisoft Connect, EA app, Rockstar Games, Battle.net, Riot Games, HoYoPlay, Amazon Games, itch.io and Heroic Games Launcher (its Epic, GOG and Amazon games). Installed games appear on the **Games** page. Choose **Find new games** after installing a game while Huedra is open.

Games that no launcher lists but that the Xbox Game Bar recognized as games while they ran are listed too, under **Game Bar**, named after their folder. A program the Game Bar took for a game by mistake can be set to **Leave HDR alone**.

For League of Legends, only the game itself counts, not the League client that stays open between matches. Test servers (Riot's PBE, Battle.net's test realms and betas) are not listed.

A game that is not listed, for example one installed without a store, can be added with **Add game**: choose its program file.

### Settings for one game

Select a game to open its details beside the list (on a narrow window they replace the list; **Back to games** returns). They are grouped under **HDR**, **HDR mod**, **Display**, **Turn HDR on before the game starts** and **About**:

| Setting | Meaning |
| --- | --- |
| **When this game runs › Turn HDR on** | HDR turns on while the game runs. The default. |
| **When this game runs › Leave HDR alone** | Huedra never switches HDR for this game. |
| **When this game runs › Let the game switch HDR** | For games with their own HDR switch: Huedra does not interfere. |
| **Wait before turning HDR on** | Waits this many seconds after the game starts. Useful for games that show a launcher first. |
| **Keep HDR on after it closes** | Seconds HDR stays on after the game closes, so restarting it, or a launcher starting the game again, does not switch the display off and on twice. |
| **Windows Auto HDR** | Whether Windows Auto HDR turns this game's SDR picture into HDR. Off unless you turn it on, here or for every game under **Settings › Auto HDR, RTX HDR and HDR mods**. Huedra sets it on the game's program (a store game's once Huedra has seen it run); it applies from the game's next start. Off and locked while the game has an HDR mod. |
| **NVIDIA RTX HDR** | Whether the NVIDIA driver turns this game's SDR picture into HDR. Off unless you turn it on, here or in Settings. Huedra sets it in the game's NVIDIA driver profile, or in a profile of its own named "*game* (Huedra)" when the driver has none for the game; it applies from the game's next start. Needs an NVIDIA graphics card; on other graphics cards the switch is unavailable. Off and locked while the game has an HDR mod. |
| **HDR mod** | A RenoDX or Luma HDR mod for the game, if one is listed: **Install**, **Remove**, or a link to its page (see [HDR mods](#hdr-mods)). |
| **Color profile in SDR / in HDR** | An ICC color profile for the game's display while it runs, one for SDR and one for HDR; the earlier profiles come back when it closes. The HDR list offers only profiles Windows applies in HDR, such as those the Windows HDR Calibration app makes. |
| **Resolution and refresh rate** | While the game runs, the display it's on uses this mode, set after HDR has turned on; Windows' own mode comes back when it closes. When HDR leaves too little bandwidth for the rate, the fastest one offered then is used. The list shows the main display's modes of the same shape as now, at 50 Hz or more. A game that sets its own refresh rate in full screen keeps its own. |

When **Game information and icons** is on, the details also show what PCGamingWiki reports about the game's HDR support and ray tracing, and may suggest a setting (**Use it** applies it). Suggestions are never applied without your choice. A game with an HDR mod gets no suggestion, since PCGamingWiki describes the game without the mod.

### Start a game with HDR already on

Some games behave badly when HDR changes while they start, and some check HDR only while they start, so their HDR settings stay grayed out when HDR came on later. For these, HDR can be turned on first:

- **Steam, recommended:** **When I press Play in Steam** (under **Turn HDR on before the game starts**) is on by default for Steam games; switch it off in the game's details if you prefer HDR to wait for the game's window. HDR turns on as soon as Steam reports the game running, about a second after you choose **Play** and before the game's window shows, on the main display until that window appears. Huedra only reads which game Steam reports as running, so this suits games with anti-cheat too.
- **Steam launch option:** the game's details in Huedra also show a launch option, under **Another way**. In Steam, right-click the game › **Properties** › **Launch options**, and paste it. It looks like this:
  ```text
  "C:\path\to\huedra.exe" launch -- %command%
  ```
  Games with anti-cheat (Battlefield 6, for example) may refuse to start this way; use the switch above instead.
- **Desktop shortcut:** the game's details have **Desktop shortcut › Create**, which starts the game with HDR.
- **Any program:** turn on **Settings › Advanced › Add "Start with HDR" to the right-click menu**. Then right-click a program in File Explorer, choose **Show more options**, and choose **Start with HDR**.

HDR stays on until the game and the programs it started have closed.

### HDR mods

Some games have no HDR, or HDR that looks wrong. [RenoDX](https://github.com/clshortfuse/renodx) and [Luma](https://github.com/Filoppi/Luma-Framework) are community projects that fix many of them: RenoDX mods are add-ons for [ReShade](https://reshade.me), and Luma mods bring their own ReShade. They are made by their own authors, not by Huedra or the game's makers.

**Finding them.** While **Settings › Auto HDR, RTX HDR and HDR mods › Look for HDR mods** is on (the default), Huedra reads both projects' lists once a day and matches them to your games by name. The cover grid marks those games **Mod available**, or **HDR mod** once one is installed; the list adds **HDR mod installed** under a game's name, and the **HDR mods** filter shows them all. The game's **HDR mod** section names the mod, its author and whether its list marks it working or in progress.

**Installing.**

1. Close the game.
2. Choose **Install** in the game's **HDR mod** section. The line below it says which program the mod goes next to. Huedra finds that program in the game's folder, so the game usually doesn't have to run first; when it can't, start the game once and Huedra learns it.
3. The first time, Huedra explains the risks (see below); check **I understand and accept these risks** to go on.
4. The next dialog lists exactly what will change: what is downloaded from where, which files are added to which folder, and that Auto HDR and RTX HDR stay off for the game. Choose **Install**. **Cancel** stops a download in progress and leaves the game as it was.

For RenoDX, Huedra downloads the newest ReShade (the add-on build) from reshade.me and adds it under the name the game loads (`dxgi.dll`, `d3d9.dll` or `opengl32.dll`), plus the mod's add-on. For Luma, it downloads the mod's zip and adds its files. Then start the game; most mods open their settings with the Home key.

Some mods can't be installed by Huedra:

- mods listed only on a page such as Nexus Mods: **Open page** takes you there;
- games that PCGamingWiki lists with anti-cheat: Huedra won't install a mod there, since changed game files can get accounts banned;
- games from the Microsoft Store, whose folders are protected;
- Vulkan games, and mods built for 32-bit games when the game is 64-bit (or the other way around).

When a game has online play, Huedra warns before the install: play offline while a mod is installed.

**Removing.** Close the game and choose **Remove**. Huedra keeps a record of every file it added and a copy of every file it replaced (in `%LOCALAPPDATA%\Huedra\mods`). Removing deletes the files it added, unless they changed since, puts back the files it replaced, and deletes the files ReShade created while the game ran (`ReShade.ini`, `ReShade.log`, `ReShadePreset.ini`) when they weren't there before. `huedra uninstall` removes every mod Huedra installed.

**Mods you installed yourself.** When you open the window, Huedra looks in your games' folders for RenoDX and Luma add-ons. A game where it finds one shows **RenoDX is installed** (or **Luma is installed**) and the folder, and Auto HDR and RTX HDR stay off for it too. Huedra doesn't touch those files unless you choose **Remove** there. Close the game first. The dialog lists everything that goes, all from the folder the add-on is in:

- the RenoDX or Luma add-on, and Luma's `Luma` folder;
- ReShade (whichever DLL's version information names ReShade, usually `dxgi.dll`) with `ReShade.ini`, `ReShade.log`, `ReShadePreset.ini` and `reshade-shaders`. When other ReShade add-ons are in the folder, ReShade and its files stay, and the dialog says so.

Huedra first copies all of it to `%LOCALAPPDATA%\Huedra\mods\removed\<game>-<number>`, then deletes it from the game's folder; the result names that folder, so you can put the files back or delete the copy. Huedra can't know whether the mod replaced one of the game's own files when you installed it, so if the game misbehaves afterwards, have Steam (**Properties › Installed Files › Verify integrity of game files**) or the game's launcher check its files.

**Risks.** Mods change game files. Online games and anti-cheat systems may treat that as cheating, and accounts have been banned for it. A mod can break with a game update, and the game's makers don't support modded games. You install mods at your own risk; Huedra's authors are not responsible for bans, crashes or lost progress.

## 7. HDR video

### Video players

Huedra watches VLC, mpv, mpv.net, MPC-HC, MPC-BE and PotPlayer. On the **Video players** page, choose for each player:

| Choice | Behavior |
| --- | --- |
| **When HDR video plays** | HDR turns on when the window title shows an HDR file. |
| **Whenever it's open** | HDR is on whenever the player is open. |
| **Never** | The player is ignored. |

- **HDR words in file names** lists the words looked for in the window title, by default `HDR`, `HDR10`, `Dolby Vision`, `DoVi` and `BT.2020`.
- **Only in full screen** turns HDR on only while the player fills the screen.
- **Keep HDR on after the video ends** keeps HDR on for a few seconds after the video ends, so the next episode does not switch the display off and on again.

### Browsers and the Huedra browser extension

Browsers change HDR only through the Huedra browser extension. With it, HDR turns on while an HDR video is open on YouTube and, if you choose, while a video plays in full screen on Netflix, Disney+, Prime Video, HBO Max or Apple TV (**Video players › Streaming sites**).

Install it:

- **Chrome, Edge, Brave, Opera, Vivaldi:** install [Huedra from the Chrome Web Store](https://chromewebstore.google.com/detail/huedra/kilkdlcbhddpigcdffdnmdknjhlddlod) and choose **Add to Chrome** (or the button named after your browser). **Get the extension** on the **Video players** page, under Browsers, opens the store page. Edge first asks to allow extensions from other stores; Opera first needs its **Install Chrome Extensions** add-on. The browser keeps the extension up to date.
- **Firefox:** from the release, download `huedra-browser-extension-<version>.zip` and unpack it into a folder you keep. Open `about:debugging#/runtime/this-firefox`, choose **Load Temporary Add-on**, and select `manifest.json` in the folder. Firefox removes temporary add-ons when it closes.

The [setup guide](SETUP.md#7-set-up-the-browser-extension) has the steps in detail, including installing by hand without the store.

After HDR turns on, the extension asks YouTube's player to switch to its HDR stream.

### Regular video with RTX Video HDR

With NVIDIA RTX Video HDR turned on in the NVIDIA app, ordinary SDR video in a browser can be shown in HDR. When **Regular video with RTX Video HDR** is on, Huedra turns HDR on while such a video plays in full screen with sound.

## 8. Displays

| Setting | Meaning |
| --- | --- |
| **Let Huedra control HDR** | One switch per display. Huedra changes HDR only on the displays it controls, in every mode; on the others HDR stays as you set it in Windows. Displays that support HDR are controlled by default. A display without HDR shows **Not available**. |
| **SDR content brightness (nits)** | Shown on a display while it is in HDR: how bright the desktop and SDR apps look, 80 to 480 nits, the same setting as Windows Settings › Display › HDR. |
| **Switch only the display with the game or video** | HDR turns on only for the display the game or video is on, and follows the window to another display. When off, every display Huedra controls switches together. |
| **Always HDR on the main display only** | Always HDR mode and the Win+Alt+H shortcut turn HDR on only for the main display, the one Windows uses as the primary display. Other displays keep following their games and video. When off, every display Huedra controls. On by default. |
| **Retries when a switch fails** | How often Huedra tries again when a display does not accept a change. |

## 9. Settings

The **Settings** page sits at the foot of the navigation pane. It was called **General** before; `huedra settings general` still opens it. The settings people change most are rows of their own; the rest fold into sections whose second line sums up their values. Click a section, or press Space or Enter on it, to open it.

| Setting | Meaning |
| --- | --- |
| **Start with Windows** | Starts Huedra in the tray when you sign in, without opening the window. |
| **Closing this window** | **Keep Huedra running** (the default): closing the window leaves Huedra switching HDR from the tray. **Quit Huedra**: closing the window closes Huedra too, and displays it turned to HDR go back to SDR. **Exit** in the tray menu always quits. |
| **Notify me when HDR doesn't switch** | A Windows notification when a display didn't switch after Huedra's retries; normal switches never notify. On by default. |
| **Keep HDR off on battery power** | For laptops: while the PC runs on battery, games and video don't turn HDR on, and HDR Huedra turned on goes off at once. Always HDR still applies. Off by default. |
| **Updates** | Ask before installing (the default), Install automatically, or Off (see [Updates](#10-updates)). |
| **Timing › Keep HDR on after a game or video ends** | Seconds HDR stays on after a game closes or a video ends, so restarting the game or playing the next video does not switch the display off and on again. Games and the Video players page can set their own time. |
| **Timing › Default wait before turning HDR on** | Seconds to wait after a game starts, unless set per game. |
| **Keyboard shortcuts** | The switch in its header turns the shortcuts in [Keyboard shortcuts](#11-keyboard-shortcuts) on or off. |
| **Auto HDR, RTX HDR and HDR mods › Windows Auto HDR for games** | The default for every game: off unless you turn it on. A game can differ in its details. Always off for games with an HDR mod. |
| **Auto HDR, RTX HDR and HDR mods › NVIDIA RTX HDR for games** | The same for NVIDIA RTX HDR. Needs an NVIDIA graphics card. |
| **Auto HDR, RTX HDR and HDR mods › Look for HDR mods** | Reads the RenoDX and Luma lists on GitHub once a day to show which games have an HDR mod (see [HDR mods](#hdr-mods)). Nothing about your games is sent. On by default. |
| **Advanced › Add "Start with HDR" to the right-click menu** | Adds Start with HDR to File Explorer's menu (see [Games](#start-a-game-with-hdr-already-on)). |
| **Advanced › Write tab and file names to the log** | Off by default, so tab names and file names stay out of the log. |
| **Advanced › Game information and icons** | Looks up games on PCGamingWiki and icons in Discord's public game list. |

The **About** section (headed with Huedra's version) has **Check for updates**, the disclaimer, the settings folder, and links to star the project on GitHub or support it on Buy Me a Coffee. Every feature is free; supporters are thanked in `SUPPORTERS.md` and get preview builds before each release.

## 10. Updates

- Huedra checks once a day. When a new version is found, the **Update available** dialog opens, but never while a game or video is using HDR: it waits until they end. It lists what is new; when there are many changes, **See the full changelog** opens the complete list on GitHub.
- **Download and install** installs the update. Huedra restarts in the tray and the settings window opens again on the new version. **Later** closes the dialog; it opens again the next day, and the tray menu offers the update meanwhile.
- With **Updates** set to **Install automatically**, there is no dialog: Huedra installs new versions on its own while no game or video is using HDR.
- Every update must carry a valid signature from the project's key, and must match the size and SHA-256 digest that the signature covers. Anything else is refused. The list of changes is signed too.
- **Check for updates** under **Settings › About** checks immediately and opens the dialog when a newer version is found.
- An update replaces only `huedra.exe`. Your settings, games and history are kept: they live in `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra` (or the `data` folder of a portable copy), which updates never touch. During the update the old program is set aside as `huedra.previous.exe`; the new version deletes it when it starts.

## 11. Keyboard shortcuts

| Keys (unless changed) | Action |
| --- | --- |
| Win+Alt+H | Turn HDR on or off |
| Win+Alt+O | Change the mode |
| Win+Alt+P | Pause or resume automatic switching (for an hour) |

Change the keys under **Settings › Keyboard shortcuts**: type them in the box beside the action, for example `Ctrl+Alt+H` or `Win+Shift+F9`, and press Enter. A shortcut needs Win, Ctrl or Alt (F13 to F24 work alone). Leave a box empty to turn that shortcut off. If another program already uses the keys, the section says so and Huedra's log names them (see [Troubleshooting](#13-troubleshooting)).

## 12. Command line

Open a terminal in Huedra's folder (in File Explorer, right-click the folder background and choose **Open in Terminal**):

```text
huedra                       Start Huedra and open its window (or bring it to the front).
huedra --background          Start Huedra in the tray only, as at sign-in.
huedra settings [page]       Open the window on a page: status, games, video, displays or settings.
huedra status                Show what the running Huedra is doing.
huedra games                 List the games found in every store.
huedra mode automatic|hdr|sdr
huedra pause [minutes]       Pause automatic switching (default 60).
huedra resume                Resume automatic switching.
huedra toggle                Turn HDR on or off, as its shortcut does.
huedra hdr brightness [NITS] [--display N | all]
                             Show or set how bright SDR content is while
                             HDR is on (80 to 480 nits).
huedra launch -- <program> [arguments]
                             Turn HDR on, start the program, keep HDR on until it ends.
huedra hdr status            List the displays and their HDR state.
huedra hdr on|off [--display N | all]
huedra update [--check]      Install a newer signed release now, or only report it.
huedra uninstall             Close Huedra, remove everything it added to the registry,
                             remove the HDR mods it installed, and put back the
                             Auto HDR and RTX HDR values games had before.
huedra exit                  Close the running Huedra.
huedra --version
```

## 13. Troubleshooting

**A display stayed in HDR (or SDR).**
Turn HDR off under **Settings › System › Display**, or run `huedra hdr off --display all`. Huedra also keeps a record of the HDR it turned on and returns those displays to SDR the next time it starts.

**The screen goes black for a moment when HDR changes.**
That is Windows changing the display mode. It takes one to two seconds and happens the same way when you switch HDR in Windows Settings.

**An HDR mod doesn't show up in the game.**
Check that the game was closed while installing, and that the details name the program you start. Some games start a different program than the one in their folder; start the game once with Huedra running so it learns the right one, then **Remove** and **Install** again. The mod's page explains its own settings and known problems.

**A game does not turn HDR on.**
Check that it is listed on the **Games** page and set to **Turn HDR on**. If it is missing, choose **Find new games**, or **Add game** and select its program. Some games start through a launcher; a **Wait before turning HDR on** of a few seconds helps.

**HDR turns on for the wrong display.**
Turn on **Displays › Switch only the display with the game or video**, and check **Let Huedra control HDR** for each display as you want.

**A browser does not turn HDR on.**
Browsers need the Huedra browser extension (see [HDR video](#7-hdr-video)). YouTube marks only videos it serves in HDR.

**Is it safe with anti-cheat software?**
Huedra reads only the name and path of the program in front and waits for programs to close. It never reads or writes another program's memory and never injects code. No game publisher has reviewed it, so this cannot be guaranteed. HDR mods are different: they change game files, so Huedra won't install one in a game PCGamingWiki lists with anti-cheat (see [HDR mods](#hdr-mods)).

**Where is the log?**
`%LOCALAPPDATA%\Huedra\huedra.log`, or `data\huedra.log` beside a portable copy. **Settings › About** shows the folder.

**Huedra closed unexpectedly.**
On the next start Huedra asks whether to copy the details. Choose **Yes**: the details are copied to the clipboard. **Report a problem...** in the tray menu does the same at any time. The details hold the Huedra and Windows versions, the last crash and the last 100 lines of the log, with your user folder shown as `%USERPROFILE%`. Huedra sends nothing itself; read them, then paste them into your problem report.

**Still stuck?**
Open an issue on the project's GitHub page and paste the details from **Report a problem...** in the tray menu, or attach the relevant part of the log. Report security problems privately, as described in [SECURITY.md](../SECURITY.md).

## 14. Portable use

Create an empty file named `portable` next to `huedra.exe`. Huedra then keeps its settings, log and caches in a `data` folder beside it, and does not add itself to Windows start-up.

## 15. Uninstall

1. Run `huedra uninstall` in Huedra's folder. It closes Huedra, removes the start-up entry and the "Start with HDR" menu entry, removes the HDR mods Huedra installed (close those games first), and puts back the Windows Auto HDR and NVIDIA RTX HDR values your games had before Huedra changed them, deleting the NVIDIA profiles Huedra made.
2. Delete Huedra's folder.
3. To remove the settings, the log and the caches as well, delete `%APPDATA%\Huedra` and `%LOCALAPPDATA%\Huedra`.

Desktop shortcuts created by Huedra are ordinary files and can be deleted like any other.

## 16. Privacy

Huedra has no accounts and no telemetry. Only the update check, the game information lookups and the HDR mod lists (and the downloads of mods you install) use the internet, and all can be turned off. The full statement, including the browser extension, is in [PRIVACY.md](../PRIVACY.md).
