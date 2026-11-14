//! The tray menu, shown with `TrackPopupMenuEx`.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CheckMenuRadioItem, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU,
    MENU_ITEM_FLAGS, MF_BYCOMMAND, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    PostMessageW, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenuEx, WM_NULL,
};
use windows::core::{HSTRING, PCWSTR};

use crate::event_loop::EventLoop;

/// One line of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    /// Text that cannot be chosen, such as the status line.
    Label(String),
    /// A command. A tab in `text` starts the right-aligned shortcut column.
    Item {
        /// Returned when chosen; must not be 0.
        id: u32,
        /// Text.
        text: String,
        /// Shows a check mark.
        checked: bool,
    },
    /// One choice of a group; the selected one shows a dot.
    Radio {
        /// Returned when chosen; must not be 0.
        id: u32,
        /// Text.
        text: String,
        /// Whether this choice is the current one.
        selected: bool,
    },
    /// A dividing line.
    Separator,
    /// An item that opens more items beside the menu.
    Submenu {
        /// Text.
        text: String,
        /// The items it opens.
        entries: Vec<MenuEntry>,
    },
}

struct Menu(HMENU);

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: created by `show` and destroyed once.
        let _ = unsafe { DestroyMenu(self.0) };
    }
}

fn append(menu: &Menu, flags: MENU_ITEM_FLAGS, id: u32, text: Option<&str>) {
    let text = text.map(HSTRING::from);
    let text = text.as_ref().map_or(PCWSTR::null(), |t| PCWSTR(t.as_ptr()));
    // SAFETY: appends to a menu owned by the caller; `text` is null or a
    // null-terminated string that outlives the call, which copies it.
    let _ = unsafe { AppendMenuW(menu.0, flags, id as usize, text) };
}

/// Adds the entries to a menu, submenus included.
fn fill(menu: &Menu, entries: &[MenuEntry]) {
    for entry in entries {
        match entry {
            MenuEntry::Label(text) => append(menu, MF_STRING | MF_GRAYED, 0, Some(text)),
            MenuEntry::Item { id, text, checked } => {
                let flags = if *checked {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                };
                append(menu, flags, *id, Some(text));
            }
            MenuEntry::Radio { id, text, selected } => {
                append(menu, MF_STRING, *id, Some(text));
                if *selected {
                    // SAFETY: the item was appended to this menu just above.
                    let _ = unsafe { CheckMenuRadioItem(menu.0, *id, *id, *id, MF_BYCOMMAND.0) };
                }
            }
            MenuEntry::Separator => append(menu, MF_SEPARATOR, 0, None),
            MenuEntry::Submenu { text, entries } => {
                // SAFETY: creates an empty menu; once appended below, the
                // parent owns it and destroys it with itself.
                let Ok(handle) = (unsafe { CreatePopupMenu() }) else {
                    continue;
                };
                let sub = Menu(handle);
                fill(&sub, entries);
                let text = HSTRING::from(text.as_str());
                // SAFETY: appends a submenu owned by `sub` to a menu owned
                // by the caller; the text is copied.
                let added =
                    unsafe { AppendMenuW(menu.0, MF_STRING | MF_POPUP, handle.0 as usize, &text) };
                if added.is_ok() {
                    // The parent destroys it now.
                    std::mem::forget(sub);
                }
            }
        }
    }
}

/// Shows the menu at the pointer and returns the chosen id, if any.
pub fn show(host: &EventLoop, entries: &[MenuEntry]) -> Option<u32> {
    // SAFETY: creates an empty menu, destroyed by the guard.
    let menu = Menu(unsafe { CreatePopupMenu() }.ok()?);
    fill(&menu, entries);
    let hwnd = host.hwnd();
    let mut point = POINT::default();
    // SAFETY: `point` is a valid out-parameter. The window must be in front
    // for the menu to close when the user clicks elsewhere.
    let chosen = unsafe {
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenuEx(
            menu.0,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY).0,
            point.x,
            point.y,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        chosen
    };
    u32::try_from(chosen.0).ok().filter(|&id| id != 0)
}
