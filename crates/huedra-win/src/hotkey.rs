//! Keyboard shortcuts, registered with `RegisterHotKey`. Shortcuts already
//! taken by another program are reported, never dropped silently.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_NOREPEAT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};

use crate::event_loop::EventLoop;

/// Huedra's shortcuts. Win+Alt+B is left to Windows, which uses it for HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    /// Win+Alt+H: turn HDR on or off.
    ToggleHdr,
    /// Win+Alt+O: change mode.
    CycleMode,
    /// Win+Alt+P: pause or resume.
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

    fn key(self) -> u32 {
        u32::from(match self {
            Self::ToggleHdr => b'H',
            Self::CycleMode => b'O',
            Self::Pause => b'P',
        })
    }

    /// The keys, as shown in menus.
    pub fn label(self) -> &'static str {
        match self {
            Self::ToggleHdr => "Win+Alt+H",
            Self::CycleMode => "Win+Alt+O",
            Self::Pause => "Win+Alt+P",
        }
    }
}

/// Registered shortcuts; unregistered on drop.
#[derive(Debug)]
pub struct Hotkeys {
    hwnd: HWND,
    registered: Vec<Hotkey>,
}

impl Hotkeys {
    /// Registers every shortcut. Returns the set and the shortcuts Windows
    /// refused because another program uses them.
    pub fn register(host: &EventLoop) -> (Self, Vec<Hotkey>) {
        let hwnd = host.hwnd();
        let mut registered = Vec::new();
        let mut refused = Vec::new();
        for key in Hotkey::ALL {
            // SAFETY: the window belongs to this thread; ids are unique.
            let ok = unsafe {
                RegisterHotKey(
                    Some(hwnd),
                    key.id(),
                    MOD_WIN | MOD_ALT | MOD_NOREPEAT,
                    key.key(),
                )
            }
            .is_ok();
            if ok {
                registered.push(key);
            } else {
                refused.push(key);
            }
        }
        (Self { hwnd, registered }, refused)
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        for key in &self.registered {
            // SAFETY: registered above for this window.
            let _ = unsafe { UnregisterHotKey(Some(self.hwnd), key.id()) };
        }
    }
}
