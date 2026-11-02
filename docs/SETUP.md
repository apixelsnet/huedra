# Huedra setup guide

A step-by-step guide to installing Huedra, checking that it works, setting up games, video players and the browser extension, and fixing the most common problems.

For a full description of every setting, see the [user guide](USER-GUIDE.md).

**Contents**

1. [Before you start](#1-before-you-start)
2. [Download and install](#2-download-and-install)
3. [First start](#3-first-start)
4. [Check that it works](#4-check-that-it-works)
5. [Set up games](#5-set-up-games)
6. [Set up video players](#6-set-up-video-players)
7. [Set up the browser extension](#7-set-up-the-browser-extension)
8. [Everyday use](#8-everyday-use)
9. [Troubleshooting](#9-troubleshooting)
10. [Asking for help](#10-asking-for-help)

---

## 1. Before you start

Check these three things first. Each takes a few seconds.

| Check | How |
| --- | --- |
| **Windows version** | Press **Win+R**, type `winver`, press **Enter**. You need Windows 10 version 1903 or later, or Windows 11, 64-bit. |
| **An HDR display** | Open **Settings › System › Display** and select the display. On Windows 11, look for **Use HDR**; on Windows 10, open **Windows HD Color settings** and look for **Use HDR** (on older versions, **Play HDR games and apps**). If the switch is there, the display supports HDR. |
| **HDR works by hand** | Turn **Use HDR** on and off once in Windows Settings. If Windows itself cannot switch HDR, Huedra cannot either; see [HDR does not work even in Windows Settings](#hdr-does-not-work-even-in-windows-settings). |

Leave HDR **off** when you are done. Huedra turns it on when it is needed.

No other software is needed. Huedra does not need administrator rights.

---

## 2. Download and install

### Step 1: Download

1. Open the project's [releases page](../../../releases).
2. Find the newest release, marked **Latest**.
3. Under **Assets**, download **`huedra-<version>-windows-x64.zip`**. This is the program.

The other files are optional:

| File | What it is |
| --- | --- |
| `huedra-<version>-windows-x64.zip.minisig` | The signature of the zip, to check the download (see step 2). |
| `huedra-browser-extension-<version>.zip` | The browser extension, for HDR video on YouTube and streaming sites (see [section 7](#7-set-up-the-browser-extension)). |

### Step 2: Check the download (optional)

Every release is signed with the project's [minisign](https://jedisct1.github.io/minisign/) key. To check it:

1. Install minisign: open **Terminal** and run
   ```powershell
   winget install jedisct1.minisign
   ```
2. Download the `.minisig` file into the same folder as the zip.
3. Run the command given in the release notes. It looks like this:
   ```powershell
   minisign -Vm huedra-1.0.1-windows-x64.zip -P <public key from the release notes>
   ```
4. **"Signature and comment signature verified"** means the file is exactly the one the project published. Anything else means the file is damaged or not genuine: delete it and download it again.

### Step 3: Choose a folder

Create a folder of your own for Huedra. The recommended place is:

```text
%LOCALAPPDATA%\Programs\Huedra
```

To create it: press **Win+R**, paste `%LOCALAPPDATA%\Programs`, press **Enter**, and create a new folder named `Huedra`.

> [!IMPORTANT]
> Do not use `C:\Program Files`. Huedra runs without administrator rights, so it cannot replace its own file there, and updates would fail. Any folder you can write to is fine.

### Step 4: Unpack

1. Right-click the downloaded zip and choose **Extract All...**.
2. Choose the folder from step 3 and choose **Extract**.

The folder now holds `huedra.exe` and a few documents.

### Step 5: Allow it past SmartScreen

Huedra does not yet have a Windows code-signing certificate, so the first start may show **"Windows protected your PC"**.

1. Choose **More info**.
2. Check that the app name is `huedra.exe`.
3. Choose **Run anyway**.

This happens only once.

---

## 3. First start

1. Double-click `huedra.exe`.
2. The Huedra window opens with a disclaimer. Huedra changes a Windows display setting, so please read it.
3. Choose **Accept and continue**. (Choosing **Exit** closes Huedra without changing anything.)

Huedra then does the following on its own:

- adds itself to Windows start-up, so it runs every time you sign in;
- finds the games installed through Steam, Epic Games, GOG, Xbox, Ubisoft Connect, EA app and Rockstar Games;
- starts in **Automatic** mode;
- places its icon in the notification area, next to the clock.

**Find the tray icon.** If you cannot see it, open the small arrow (**^**) beside the clock. Drag the Huedra icon onto the taskbar to keep it visible.

| Icon | Meaning |
| --- | --- |
| Coloured gamut shape | HDR is on |
| Grey outline | SDR (HDR off) |

**Closing the window keeps Huedra running** in the notification area. To bring the window back, left-click the tray icon, or start `huedra.exe` again. To close Huedra completely, right-click the tray icon and choose **Exit**.

---

## 4. Check that it works

A quick test, without starting a game:

1. Open the Huedra window and go to the **Status** page.
2. Press **Win+Alt+H**. The screen goes black for a second or two, and the main display switches to HDR. The tray icon turns coloured.
3. Press **Win+Alt+H** again. The display returns to SDR and the icon turns grey.

If that worked, Huedra can switch your display. Automatic switching for games and video works the same way.

### Optional: check from the command line

1. Open Huedra's folder in File Explorer.
2. Right-click an empty part of the folder and choose **Open in Terminal**.
3. Run:
   ```powershell
   .\huedra.exe hdr status
   ```
   This lists your displays and whether each one is in HDR.
4. Run:
   ```powershell
   .\huedra.exe status
   ```
   This shows what the running Huedra is doing: its mode, the state of each display, and the recent changes.

> [!NOTE]
> In PowerShell and Windows Terminal, type `.\huedra.exe`, not just `huedra`: PowerShell does not run programs from the current folder without the `.\` in front.

---

## 5. Set up games

Most games need no setup: they are found automatically and set to **Enable while running**.

### Check that your games are listed

1. Open the **Games** page.
2. Look for your game. If you installed it after Huedra started, choose **Scan again**.

### Add a game that is not listed

Games installed without a store, or from a store Huedra does not read, can be added by hand:

1. On the **Games** page, choose **Add game**.
2. Select the game's program file (`.exe`). It is usually in the game's installation folder, not the launcher's.

### Choose how a game is treated

Select a game to open its settings:

| Setting | Use it when |
| --- | --- |
| **Enable while running** | The normal choice. HDR is on while the game runs. |
| **Do not change** | You never want Huedra to switch HDR for this game. |
| **Game controls HDR** | The game has its own HDR switch and turns Windows HDR on by itself. |
| **Delay before enabling HDR** | The game shows a launcher or splash screen first, or crashes if HDR changes while it starts. Try 5 to 10 seconds. |
| **Keep HDR on after the game closes** | The game restarts itself, or its launcher starts it again, and the display would otherwise switch off and on twice. |

### Games that need HDR on before they start

Some games misbehave when HDR changes while they start. For those, start the game with HDR already on:

- **Steam:**
  1. Open the game's page in Huedra and copy the **Steam launch option**.
  2. In Steam, right-click the game › **Properties** › **General** › **Launch options**.
  3. Paste it. It looks like this:
     ```text
     "C:\path\to\huedra.exe" launch -- %command%
     ```
- **Desktop shortcut:** on the game's page, choose **Create a desktop shortcut**, and start the game from it.
- **Any program:** turn on **General › "Start with HDR" in the menu of programs**. Then right-click the program in File Explorer, choose **Show more options**, and choose **Start with HDR**.

HDR stays on until the game, and every program it started, has closed.

---

## 6. Set up video players

Huedra watches **VLC, mpv, mpv.net, MPC-HC, MPC-BE and PotPlayer**. It recognises HDR video by words in the player's window title, which normally shows the file name.

1. Open the **Video players** page.
2. Under **Players**, choose for each player:

   | Choice | Behaviour |
   | --- | --- |
   | **When HDR video plays** | HDR turns on when the title shows an HDR file. The recommended choice. |
   | **While open** | HDR is on whenever the player is open. |
   | **Never** | The player is ignored. |

3. Check **Words that mark HDR video**. The defaults are `HDR`, `HDR10`, `Dolby Vision`, `DoVi` and `BT.2020`. Add any word your files use, separated by commas, and press **Enter**.
4. **Only in full screen** (on by default): HDR turns on only while the player fills the screen. A maximised window does not count; use the player's full-screen mode (usually **F** or a double-click).

**Test it:** play a file whose name contains `HDR` (for example `Film.2160p.HDR10.mkv`) in full screen. The display switches to HDR. Leave full screen or close the player, and it returns to SDR after a few seconds.

> [!TIP]
> The player must show the file name in its title bar. In VLC this is the default; in mpv and others, check that the title is not set to something fixed.

---

## 7. Set up the browser extension

Browsers do not tell Windows when a video is in HDR. The **Huedra browser extension** fills that gap. With it:

- **YouTube:** HDR turns on while a video that YouTube offers in HDR is open in full screen, and the player then switches to its HDR stream. (With **Video players › Only in full screen** turned off, the video does not need to be in full screen.)
- **Netflix, Disney+, Prime Video, HBO Max, Apple TV:** if you choose, HDR turns on while a video plays in full screen on these sites.

The extension has no settings, no buttons and no permissions. It sends nothing over the network and stores nothing. It works by adding a short marker to the start of the tab title (`⟦HDR⟧` on YouTube, `⟦Video netflix⟧` and similar on streaming sites), which Huedra reads on your PC.

### Step 1: Get the extension

The extension has been submitted to the browser stores. Until it appears there, install it by hand:

1. From the newest [release](../../../releases), download **`huedra-browser-extension-<version>.zip`**.
2. Create a folder you will keep, for example `%LOCALAPPDATA%\Programs\Huedra\extension`.
3. Extract the zip into that folder. The folder must contain `manifest.json` directly, not inside another folder.

> [!IMPORTANT]
> Do not delete or move this folder afterwards. The browser loads the extension from it every time it starts.

### Step 2: Install it in your browser

#### Chrome, Edge, Brave, Opera, Vivaldi

1. Open the browser's extensions page by typing its address into the address bar:

   | Browser | Address |
   | --- | --- |
   | Chrome | `chrome://extensions` |
   | Edge | `edge://extensions` |
   | Brave | `brave://extensions` |
   | Opera | `opera://extensions` |
   | Vivaldi | `vivaldi://extensions` |

2. Turn on **Developer mode**. In Chrome, Brave, Opera and Vivaldi it is at the top right; in Edge it is in the left-hand panel.
3. Choose **Load unpacked**.
4. Select the folder from step 1 (the one containing `manifest.json`) and choose **Select Folder**.
5. **Huedra** appears in the list of extensions. Make sure its switch is on.

Chrome may show a reminder about developer-mode extensions when it starts. That is expected for extensions loaded this way; keep the extension enabled.

#### Firefox

Firefox 128 or later is needed.

1. Type `about:debugging#/runtime/this-firefox` into the address bar.
2. Choose **Load Temporary Add-on...**.
3. Open the extension folder and select **`manifest.json`**.

> [!NOTE]
> Firefox removes temporary add-ons when it closes, so these steps must be repeated after each restart until the extension is available from Firefox Add-ons.

### Step 3: Check the browser in Huedra

1. Open Huedra's **Video players** page.
2. Under **Browsers**, make sure your browser is set to **When HDR video plays** (the default), not **Never**.

### Step 4: Turn on streaming sites (optional)

Netflix, Disney+, Prime Video, HBO Max and Apple TV do not reveal whether a video is in HDR, so Huedra cannot tell. You decide which of them count:

1. On the **Video players** page, scroll to **Streaming sites**.
2. Turn on each site that streams HDR to your browser. (Many sites stream HDR only to some browsers and only with the right subscription; check the site's help pages.)

On a site turned on here, a video playing in **full screen** turns HDR on, and leaving full screen turns it off again. Pausing does not switch the display.

### Step 5: Test it

1. Open an HDR video on YouTube. Searching for "4K HDR" finds many.
2. Look at the browser tab: its title starts with **`⟦HDR⟧`**. That shows the extension is working.
3. Put the video in full screen (press **F**). Within a moment the display switches to HDR. YouTube reloads the video at the same point and plays its HDR stream; the quality menu (gear icon) now shows **HDR** next to the resolution.
4. Leave full screen or close the tab. The display returns to SDR after a few seconds.

### Updating the extension

When a new release brings a new extension version:

1. Extract the new zip over the same folder, replacing the old files.
2. **Chrome, Edge and others:** on the extensions page, choose the **reload** arrow on the Huedra card.
   **Firefox:** load it again as in step 2.

### Removing the extension

On the browser's extensions page, choose **Remove** on the Huedra card, then delete the extension folder.

---

## 8. Everyday use

Nothing needs to be done day to day. Huedra starts when you sign in and switches HDR on its own.

| To... | Do this |
| --- | --- |
| Open the window | Left-click the tray icon, or start `huedra.exe`. |
| Turn HDR on or off by hand | **Win+Alt+H** |
| Change the mode (Automatic, Always HDR, Always SDR) | **Win+Alt+O**, or right-click the tray icon. |
| Pause automatic switching for an hour | **Win+Alt+P**, or right-click the tray icon › **Pause for 1 hour**. |
| Stop Huedra switching for the game in front | Right-click the tray icon › **Never for *game***. |
| Close Huedra | Right-click the tray icon › **Exit**. Displays Huedra turned to HDR go back to SDR. |

Updates: Huedra checks once a day. When a new version is found, the **Update available** dialog asks before installing, and never while a game or video is using HDR. Settings and games are kept.

---

## 9. Troubleshooting

Find the symptom, then work through the checks in order.

### Installing and starting

#### Windows says "Windows protected your PC"

Choose **More info**, then **Run anyway**. Huedra is not yet signed with a Windows code-signing certificate. To make sure the file is genuine, check its signature as in [step 2](#step-2-check-the-download-optional).

#### The antivirus removes or blocks `huedra.exe`

1. Check the zip's signature as in [step 2](#step-2-check-the-download-optional). If it is verified, the file is exactly the one the project published.
2. Restore the file from the antivirus quarantine and allow it.
3. Please [report it](#10-asking-for-help) with the antivirus name and the detection name, so it can be reported to the vendor as a false positive.

#### Nothing happens when I start `huedra.exe`

1. Huedra may be running already, with its window hidden. Look for the tray icon beside the clock (open the **^** arrow).
2. Open **Task Manager** (**Ctrl+Shift+Esc**) and look for `huedra.exe`. If it is listed but no window or icon appears, choose **End task**, then start Huedra again.
3. If it still does not start, see [Asking for help](#10-asking-for-help) and include the log.

#### I cannot find the tray icon

Open the small arrow (**^**) beside the clock. To keep the icon visible, drag it onto the taskbar, or open **Settings › Personalization › Taskbar › Other system tray icons** and turn Huedra on.

#### Huedra does not start with Windows

1. Check **General › Start with Windows** in Huedra.
2. Open **Task Manager › Startup apps** and make sure Huedra is **Enabled**.
3. A portable copy (a file named `portable` next to `huedra.exe`) never adds itself to start-up.

### HDR switching

#### HDR does not work even in Windows Settings

Huedra uses the same switch as Windows, so fix this first:

1. Update the graphics driver from NVIDIA, AMD or Intel.
2. Check that HDR is enabled in the display's own on-screen menu (often called HDR, HDMI Ultra HD Deep Colour, or Enhanced format).
3. Use a cable and port that support HDR: DisplayPort 1.4, or HDMI 2.0 or later.
4. On laptops, plug in the charger: some laptops do not allow HDR on battery. See **Settings › System › Display › HDR › Battery options**.

#### The screen goes black for a moment when HDR changes

This is normal. Windows changes the display mode, which takes one to two seconds, exactly as when you switch HDR in Windows Settings.

#### A display stayed in HDR (or in SDR)

1. Press **Win+Alt+H** to switch it back, or turn HDR off under **Settings › System › Display**.
2. Or, in a terminal in Huedra's folder:
   ```powershell
   .\huedra.exe hdr off --display all
   ```

Huedra also remembers which displays it turned to HDR, and returns them to SDR the next time it starts.

#### Huedra turned HDR off that I had turned on myself

It should not: Huedra only turns off HDR it turned on itself. If you want HDR to stay on, use **Always HDR** mode, or pause Huedra. If it still happens, please [report it](#10-asking-for-help).

#### HDR turns on for the wrong display, or for every display

1. Open the **Displays** page.
2. Turn on **Only the display showing the content**: HDR then turns on only for the display the game or video is on, and follows the window to another display.
3. Check that each display is **Managed** as you want. A display that is not managed is never switched.
4. **Always HDR** mode and **Win+Alt+H** switch only the main display unless **Always HDR on the main display only** is turned off.

#### The shortcuts Win+Alt+H, O or P do nothing

1. Check that **General › Keyboard shortcuts** is on.
2. If another program already uses a shortcut, Huedra's log says so: "The shortcut Win+Alt+H is used by another program and is not available." (see [Find the log by hand](#find-the-log-by-hand)). Change or close that program, then turn the setting off and on again.

### Games

#### A game does not turn HDR on

Work through these in order:

1. **Mode:** the **Status** page must show **Automatic** mode, not paused.
2. **Listed:** the game must be on the **Games** page. If not, choose **Scan again**, or **Add game** and select its program file.
3. **Setting:** the game must be set to **Enable while running**. Choosing **Never for *game*** in the tray menu changes this; set it back on the game's page.
4. **Launcher:** if the game starts through a launcher, the launcher may be what Huedra sees first. Set a **Delay before enabling HDR** of 5 to 10 seconds. If the game's real program has a different name from the one listed, add it with **Add game**.
5. **Display:** the display the game is on must be **Managed** on the **Displays** page.

#### A game crashes, shows wrong colours, or starts in SDR when HDR turns on

The game does not cope with HDR changing while it starts. Either:

- set a **Delay before enabling HDR** so HDR changes after it has started, or
- start it with HDR already on, through the Steam launch option, a desktop shortcut, or **Start with HDR** (see [section 5](#games-that-need-hdr-on-before-they-start)).

#### HDR switches off and on twice when a game starts or restarts

Raise **Keep HDR on after the game closes** for that game, or the default under **General › Keep HDR on after a game or video ends**.

#### Is Huedra safe with anti-cheat software?

Huedra reads only the name and path of the program in front and waits for programs to close. It never reads or writes another program's memory and never injects code. No game publisher has reviewed it, so this cannot be guaranteed.

### Video players

#### An HDR file does not turn HDR on

1. The player must be set to **When HDR video plays** (or **While open**) on the **Video players** page.
2. The player's title bar must show the file name, and the file name must contain one of the **Words that mark HDR video**. Whole words count: `Film.HDR10.mkv` matches, `HDRezka.mkv` does not match `HDR`. Add the word your files use.
3. With **Only in full screen** on, the player must be in true full-screen mode. A maximised window does not count.

#### HDR turns off between episodes

Raise **Keep HDR on after the video ends** on the **Video players** page.

### Browser and extension

#### The tab title does not show `⟦HDR⟧` on YouTube

1. **Extension installed and on:** open the browser's extensions page and check that **Huedra** is listed and its switch is on.
2. **Firefox only:** the extension disappears when Firefox closes. Load it again (see [Firefox](#firefox)).
3. **The video is in HDR:** YouTube marks only videos it offers in HDR. Try a video with "HDR" in its name.
4. **Reload the YouTube tab** after installing or reloading the extension.
5. **Browser version:** Chrome, Edge and similar need version 111 or later; Firefox needs 128 or later.
6. **Folder moved or deleted:** the extensions page shows an error on the Huedra card. Extract the zip again into a folder you keep and load it again.

#### The tab shows `⟦HDR⟧`, but HDR does not turn on

1. On Huedra's **Video players** page, under **Browsers**, check that your browser is set to **When HDR video plays**, not **Never**. Choosing **Never for *browser*** in the tray menu sets it to **Never**.
2. Huedra must be in **Automatic** mode and not paused.
3. If **Only in full screen** is on, put the video in full screen (press **F** on YouTube).
4. The display the browser is on must be **Managed** on the **Displays** page.

#### HDR is on, but YouTube still looks like SDR

1. Wait a moment: after the switch, YouTube reloads the video at the same point to load its HDR stream.
2. Open the quality menu (gear icon). The resolution should show **HDR**. If not, choose a quality with **HDR** by hand.
3. If **HDR** never appears, check that the browser's hardware acceleration is on: in Chrome, **Settings › System › Use graphics acceleration when available**; in Edge, **Settings › System and performance › Use graphics acceleration when available**.

#### Netflix, Disney+, Prime Video, HBO Max or Apple TV does not turn HDR on

1. On Huedra's **Video players** page, under **Streaming sites**, the site must be turned on. All are off by default.
2. The video must be in **full screen**, using the site's own full-screen button. A maximised browser window does not count.
3. Check that the tab title starts with `⟦Video ...⟧` while the video is in full screen. If not, check that the extension is installed and on, and reload the page.
4. The site itself may not stream HDR to your browser or subscription. Huedra only switches the display; it cannot make the site send HDR.

#### Chrome warns about developer-mode extensions

This is expected while the extension is loaded unpacked. Keep it enabled. The warning goes away once the extension is installed from the Chrome Web Store.

### Updates

#### An update fails to install

1. Huedra's folder must be writable. If it is in `C:\Program Files`, move the whole folder to `%LOCALAPPDATA%\Programs\Huedra`, start it from there, and check again with **General › About › Check for updates**.
2. An update is refused when its signature is not valid. That protects you; try again later, or download the newest release from GitHub by hand.
3. A leftover `huedra.previous.exe` in the folder is deleted by the new version when it starts. It is safe to delete by hand once Huedra is closed.

#### No update dialog appears

The dialog never opens while a game or video is using HDR; it waits until they end. Check that **General › Updates** is not set to **Off**, or choose **General › About › Check for updates**.

---

## 10. Asking for help

If none of the above helps, open an issue on the project's [GitHub page](../../../issues/new/choose) with the details below.

### Copy the problem details

Right-click the tray icon and choose **Report a problem...**. Huedra copies the details to the clipboard:

- the Huedra and Windows versions;
- the last crash, if any;
- the last 100 lines of the log, with your user folder shown as `%USERPROFILE%`.

Huedra sends nothing itself. Read the details, then paste them into the issue.

If Huedra closed unexpectedly, it offers to copy the same details the next time it starts. Choose **Yes**.

### Find the log by hand

| Copy | Log file |
| --- | --- |
| Normal | `%LOCALAPPDATA%\Huedra\huedra.log` |
| Portable | `data\huedra.log`, next to `huedra.exe` |

**General › About** shows the folder. Window titles (tab names and file names) are not written to the log unless **General › Record window titles in the log** is on; turn it on only while reproducing a video or browser problem, and off again afterwards.

### Also include

- your graphics card and driver version;
- the displays involved and how they are connected (HDMI or DisplayPort);
- for a game: its name and store; for video: the player or browser, and the site.

Report security problems privately, as described in [SECURITY.md](../SECURITY.md), not in a public issue.
