# 15 — Design

How every page, control and message in Huedra looks and behaves. Huedra is a Windows 11 app first: it follows WinUI (Fluent) conventions so it feels like part of Windows. New features start from this file; a feature that needs something not covered here adds it here first.

## Principles
- **Native first.** Use the Windows control for the job (setting row, toggle switch, InfoBar, dialog) before inventing one. Huedra's own identity lives in the logo, the Status card's spectrum and the pink HDR labels, nowhere else.
- **Start from the screen that exists.** A new feature adds rows, columns or sections to today's pages; it does not bring a new layout.
- **One main action per view.** Only that button uses the accent color.
- **Huedra never changes a setting on its own.** Anything that writes outside Huedra shows what will change first, and can be undone.
- **Every view works** by keyboard, in light and dark, in all three layouts, at 100 % to 200 % scaling (see 08).

## Color
Neutral layers as in Windows, one fixed accent, and the Windows status colors. The accent is Windows' default blue, fixed: following the user's accent color would let it turn pink, which here means HDR.

| Token | Light | Dark | Use |
|---|---|---|---|
| base | `#F3F3F3` | `#202020` | Window background where Mica isn't available (before Windows 11 22H2) |
| card | `#FBFBFB` | `#2B2B2B` | Setting rows, tables, cards |
| card-stroke | `rgba(0,0,0,.07)` | `rgba(0,0,0,.28)` | Card borders |
| control | `#FDFDFD` | `#2D2D2D` | Buttons, drop-downs, text boxes |
| control-hover | `#F6F6F6` | `#323232` | Hover on controls and clickable rows |
| control-stroke | `rgba(0,0,0,.07)`, bottom `rgba(0,0,0,.18)` | `rgba(255,255,255,.07)`, bottom `rgba(255,255,255,.04)` | Control borders (darker bottom edge in light) |
| strong-stroke | `rgba(0,0,0,.6)` | `rgba(255,255,255,.6)` | Toggle and check box outlines, text box underline |
| divider | `rgba(0,0,0,.08)` | `rgba(255,255,255,.08)` | Lines between rows |
| subtle-hover / subtle-selected | `rgba(0,0,0,.04)` / `.06` | `rgba(255,255,255,.05)` / `.07` | Navigation, list rows, icon buttons |
| text / text-secondary / text-tertiary | `rgba(0,0,0,.9)` / `.62` / `.45` | `#FFFFFF` / `rgba(255,255,255,.78)` / `.54` | Body, descriptions, disabled |
| accent / on-accent | `#005FB8` / `#FFFFFF` | `#60CDFF` / `#000000` | Main action, switches on, selection marks, links |
| success, background | `#0F7B0F`, `#DFF6DD` | `#6CCB5F`, `#393D1B` | Done, installed |
| caution, background | `#9D5D00`, `#FFF4CE` | `#FCE100`, `#433519` | Warnings |
| critical, background | `#C42B1C`, `#FDE7E9` | `#FF99A4`, `#442726` | Errors, blocked actions |
| hdr-label, background | `#C4174F`, `rgba(255,45,111,.12)` | `#FF6F9A`, `rgba(255,45,111,.18)` | HDR labels only |
| dialog / dialog-footer | `#FFFFFF` / `#F3F3F3` | `#2B2B2B` / `#202020` | Dialogs |
| smoke | `rgba(0,0,0,.3)` | `rgba(0,0,0,.55)` | Behind a dialog |

The spectrum colors (BT.2020) stay for the logo and the Status card while HDR is on, gray in SDR.

**Mica.** On Windows 11 22H2 and later the window asks for the Mica backdrop and leaves the navigation pane and the page area transparent, so Windows tints both with the wallpaper (see 08, since 1.2.0). What sits on the window uses WinUI's see-through fills (`theme.slint`), so the tint shows through cards and controls too, as in Windows' own Settings:

| Token | Light | Dark | WinUI name | Use |
|---|---|---|---|---|
| `card` / `card-stroke` | `rgba(255,255,255,.7)` / `rgba(0,0,0,.06)` | `rgba(255,255,255,.05)` / `rgba(0,0,0,.1)` | CardBackgroundFillColorDefault / CardStrokeColorDefault | Cards, setting rows, the details pane's rows, expanders, the Status card |
| `control-fill` / `-hover` / `-pressed` / `-disabled` | `rgba(255,255,255,.7)` / `rgba(249,249,249,.5)` / `rgba(249,249,249,.3)` / `rgba(249,249,249,.3)` | `rgba(255,255,255,.06)` / `.08` / `.03` / `.04` | ControlFillColor… | Buttons, drop-downs, text boxes, key labels, filter chips |
| `control-stroke` | `rgba(0,0,0,.06)` | `rgba(255,255,255,.07)` | ControlStrokeColorDefault | Their borders |
| `input-active` | `#FFFFFF` | `rgba(30,30,30,.7)` | ControlFillColorInputActive | A text box with the keyboard |
| `subtle` / `subtle-pressed` | `rgba(0,0,0,.035)` / `.024` | `rgba(255,255,255,.06)` / `.04` | SubtleFillColorSecondary / Tertiary | Navigation and list rows (hover and selected), icon buttons, tiles and insets on a card, the mode selector's track |

Over the solid base (no Mica, or an inactive window, which Windows draws plain) these add up to the solid tokens of the first table: a card comes out at `#2B2B2B` / `#FBFBFB`, a tile on a card at about `#383838`. Text keeps its contrast, as in Settings. Things that float over the window keep solid fills: dialogs, the first-run setup, drop-down lists, and the quick panel with its buttons (a long label there fades into its button's solid color). New pages follow the same split: background transparent, content on see-through cards.

**Status card.** A banner of glowing waves across its top (96 px), fading into the card with no line between, then the color chart and the status, as tall as they need (at least 128 px); the same shape in SDR and HDR. The waves are gray in SDR and in the spectrum's colors in HDR; on a display in HDR mode they may be drawn in real HDR by a layer over the banner (see 08), so nothing is drawn under the banner but its waves. Faint gradients are drawn in Rust, dithered and unscaled: drawn by the renderer at low opacity they show bands. Hover lifts the chart, a click sends a ripple, nothing more; no motion with Windows animation effects off.

## Type
Segoe UI Variable, sizes in effective pixels:

| Style | Size / line | Weight | Use |
|---|---|---|---|
| Caption | 12 / 16 | Regular | Descriptions, table headers, small print |
| Body | 14 / 20 | Regular | Everything else |
| Body strong | 14 / 20 | Semibold | Section headers, emphasis inside messages |
| Subtitle | 20 / 28 | Semibold | Item name in the details pane, dialog titles |
| Title | 28 / 36 | Semibold | Page titles |

Sentence case everywhere. No other sizes.

## Shape and spacing
- Corners: 8 px on the window, dialogs, flyouts, tables and covers in the cover grid; 4 px on buttons, text boxes, list rows, setting rows, tags and InfoBars.
- Spacing on a 4 px grid: 4 px between setting rows in a group, 12 px between blocks of a page, 14–18 px above a section header, 24 px inside dialogs, 28 px page padding.
- Icons: Segoe Fluent Icons, 16 px in controls and navigation, 20 px in setting rows.

## Layout
- Navigation on the left (240 px, 200 px medium, icon rail compact); Settings pinned at the foot. Selected item: subtle fill and a 3 x 16 px accent mark.
- Page: title, then a toolbar (search grows, buttons, view switch), then filter chips, then content.
- List and details: the list on the left, the details pane on the right (420 px) while an item is open; on narrow layouts the pane replaces the list with a back button. The list hides columns rather than squeezing names.
- The details pane is a column of setting rows under section headers, never a card holding stacked labels.

## Components
| Component | Use for | Rules |
|---|---|---|
| Setting row | One setting | Icon, title, description (caption) on the left; the control on the right. 68 px minimum, 60 px in the details pane; taller when the text wraps (the minimum is never a fixed height). Grouped under a section header, four or five per group. |
| Expandable setting row | A setting with details or a list | Same header; a chevron opens a body of sub-rows separated by dividers, indented to the title. |
| Toggle switch | On or off | 40 x 20 px; "On" or "Off" text to its left in a setting row. Disabled, not hidden, when something else holds it, and the description says why. |
| Drop-down | One choice of several | On the right of its row, 150–200 px wide. |
| Button | Actions | Standard by default; accent for the one main action. Destructive actions are standard buttons that open a confirmation dialog. |
| Hyperlink button | Opening a web page | Accent text with the open-in-new-window icon. |
| Check box | An acknowledgment the user must give | Enables the action it guards. |
| InfoBar | Status that stays in the page | Severity icon, bold title, message, optional action and close button. Info (accent), success, warning (caution), error (critical). Blocking errors have no close button. Never a floating toast. |
| Dialog | A decision or confirmation | One at a time. Title (subtitle style), what will happen in plain words, the main action (accent) first and Cancel last, equal widths, in a footer band. Keeps the keyboard: focus starts inside, Tab and Shift+Tab go round its controls only, Escape is Cancel. |
| Progress | Work that takes seconds | Progress bar in the dialog with the current step in words and "Step n of m"; Cancel undoes what was done. |
| Table | Lists of games, apps, displays | Card with a caption header row, 56 px rows, hover fill, selected fill with the accent mark. |
| Cover grid | Games by cover | 2 : 3 covers; HDR label top left, one status tag bottom left. |
| Tag | A short status on an item | 20 px, 4 px corners; neutral, HDR (pink) or success (green). |
| Filter chips | Filtering a list | 28 px pills with a count; the selected one in the accent. |

## Writing
US English, plain and short. Buttons say what they do ("Install", "Remove", "Check for updates"). Descriptions say what the setting does, not how it works. Messages say what happened and what to do next, without apologizing. Anything that writes outside Huedra, such as game files or graphics driver settings, lists exactly what will change before it happens.

## Scope (moving today's app to this design)
- `theme.slint`: neutral layers instead of the violet-tinted ones, the fixed Windows blue accent instead of ultramarine, the caution color for warnings (ending the "no yellow" rule), pink for HDR labels only.
- `controls.slint`: `Toggle` with On/Off text, a setting row component and an expandable one, InfoBar, dialog footer with equal-width buttons, progress bar, tag.
- `pages.slint`: the details pane and the Settings page as setting rows under section headers; status banners as InfoBars.
- Mica is already in place (08) and stays as it is.

## Done when
Every page uses these tokens and components, checked in light and dark, in all three layouts and at 100 %, 150 % and 200 % scaling; the exe stays under the size cap; the README screenshots and tour are retaken.

## Result (2026-11-16) — moved to this design
Code: `crates/huedra-ui/ui/{theme,controls,pages,app}.slint`.
- Done as scoped above: Windows neutrals and the fixed blue accent, caution color, pink for HDR labels only; `Toggle` with On/Off text, `PaneRow` and `SubRow` setting rows, the expandable section, InfoBar kinds, dialog footer with equal-width buttons, progress bar, success tag; the details pane and the Settings page as setting rows.
- Rows and InfoBars take at least their layout's measured height, so wrapped text makes them taller at any window width (see 08).
- The README tour and Games screenshot were retaken with the sample build.

Verified on this PC: dark theme at 100 % in the wide, medium and compact layouts (window widths 1000 to 540 px).
Not verified: light theme and 150 %/200 % scaling of the new rows and dialogs.
