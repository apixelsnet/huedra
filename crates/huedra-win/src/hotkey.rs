//! Keyboard shortcuts, registered with `RegisterHotKey`. Shortcuts already
//! taken by another program are reported, never dropped silently.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

use huedra_core::shortcut::{Modifiers, Shortcut};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey,
};

use crate::event_loop::EventLoop;

/// Huedra's shortcuts; their keys come from the settings
/// (`huedra_core::shortcut`). Win+Alt+B is left to Windows, which uses it
/// for HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    /// Turn HDR on or off (Win+Alt+H unless changed).
    ToggleHdr,
    /// Change the mode (Win+Alt+O unless changed).
    CycleMode,
    /// Pause or resume (Win+Alt+P unless changed).
    Pause,
}

impl Hotkey {
    /// Every shortcut.
    pub const ALL: [Self; 3] = [Self::ToggleHdr, Self::CycleMode, Self::Pause];

    fn id(self) -> i32 {
        match self {
            Self::ToggleHdr => 1,
            Self::CycleMode => 2,
            Self::Pause => 3,
        }
    }

    pub(crate) fn from_id(id: usize) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| usize::try_from(k.id()).is_ok_and(|own| own == id))
    }
}

/// Registered shortcuts; unregistered on drop.
#[derive(Debug)]
pub struct Hotkeys {
    hwnd: HWND,
    registered: Vec<Hotkey>,
}

impl Hotkeys {
    /// Registers the shortcuts given, in the order of [`Hotkey::ALL`];
    /// `None` leaves that one off. Returns the set and the shortcuts
    /// Windows refused because another program uses them.
    pub fn register(host: &EventLoop, keys: [Option<Shortcut>; 3]) -> (Self, Vec<Hotkey>) {
        let hwnd = host.hwnd();
        let mut registered = Vec::new();
        let mut refused = Vec::new();
        for (action, keys) in Hotkey::ALL.into_iter().zip(keys) {
            let Some(keys) = keys else {
                continue;
            };
            // SAFETY: the window belongs to this thread; ids are unique.
            let ok = unsafe {
                RegisterHotKey(
                    Some(hwnd),
                    action.id(),
                    modifiers(keys.modifiers()) | MOD_NOREPEAT,
                    keys.key().virtual_key(),
                )
            }
            .is_ok();
            if ok {
                registered.push(action);
            } else {
                refused.push(action);
            }
        }
        (Self { hwnd, registered }, refused)
    }
}

/// The `RegisterHotKey` flags for a shortcut's modifier keys.
fn modifiers(held: Modifiers) -> HOT_KEY_MODIFIERS {
    [
        (Modifiers::WIN, MOD_WIN),
        (Modifiers::CTRL, MOD_CONTROL),
        (Modifiers::ALT, MOD_ALT),
        (Modifiers::SHIFT, MOD_SHIFT),
    ]
    .into_iter()
    .filter(|(m, _)| held.contains(*m))
    .fold(HOT_KEY_MODIFIERS(0), |all, (_, flag)| all | flag)
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        for key in &self.registered {
            // SAFETY: registered above for this window.
            let _ = unsafe { UnregisterHotKey(Some(self.hwnd), key.id()) };
        }
    }
}
