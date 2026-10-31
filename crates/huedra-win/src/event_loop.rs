//! The tray process's hidden window and message loop.
//!
//! The window procedure and the WinEvent callback only queue events; the
//! caller's handler runs between messages, never inside a callback, so it may
//! open menus or switch displays safely. Feature docs: `03-watchers.md`,
//! `07-tray-hotkeys.md`.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::size_of;
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::NIN_SELECT;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer, MSG,
    PostMessageW, RegisterClassExW, RegisterWindowMessageW, SetTimer, TranslateMessage,
    WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU, WM_DISPLAYCHANGE, WM_HOTKEY, WM_TIMER, WNDCLASSEXW,
    WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result, w};

use crate::hotkey::Hotkey;

pub(crate) const WM_TRAY: u32 = WM_APP + 1;
pub(crate) const WM_PROCESS_EXIT: u32 = WM_APP + 2;
pub(crate) const WM_WAKE: u32 = WM_APP + 3;
/// `NIN_KEYSELECT`: the icon was chosen with the keyboard.
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const CLASS_NAME: PCWSTR = w!("HuedraTray");
/// Shortest timer Windows supports (`USER_TIMER_MINIMUM`).
const MIN_TIMER_MS: u32 = 10;

/// Something the tray process must react to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A window of this process id came to the front.
    Foreground(u32),
    /// A watched process exited.
    ProcessExited(u32),
    /// A top-level window of a watched video program changed its title or
    /// position (see [`crate::window::ProcessHook`]).
    WindowChanged(u32),
    /// A timer set with [`EventLoop::set_timer`] fired.
    Timer(TimerId),
    /// The display configuration changed (`WM_DISPLAYCHANGE`).
    DisplayChanged,
    /// A keyboard shortcut was pressed.
    Hotkey(Hotkey),
    /// The tray icon was clicked or chosen with the keyboard.
    TrayClick,
    /// The tray icon's menu was requested.
    TrayMenu,
    /// Explorer restarted; the tray icon must be added again.
    TaskbarCreated,
    /// Another thread has work for the loop (see [`Waker`]).
    Wake,
}

/// Identifies a timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerId(pub usize);

/// Whether the loop continues after an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Keep running.
    Continue,
    /// Leave [`EventLoop::run`].
    Exit,
}

thread_local! {
    static QUEUE: RefCell<VecDeque<Event>> = const { RefCell::new(VecDeque::new()) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static WINDOW: Cell<isize> = const { Cell::new(0) };
}

/// Queues an event and makes sure `GetMessageW` returns to process it.
/// Window changes already waiting in the queue are not queued twice: a
/// window being dragged reports every step.
pub(crate) fn push(event: Event) {
    let queued = QUEUE.with_borrow_mut(|q| {
        if matches!(event, Event::WindowChanged(_)) && q.contains(&event) {
            return false;
        }
        q.push_back(event);
        true
    });
    if !queued {
        return;
    }
    let window = WINDOW.get();
    if window != 0 {
        Waker(window).post(WM_WAKE, 0);
    }
}

fn pop() -> Option<Event> {
    QUEUE.with_borrow_mut(VecDeque::pop_front)
}

/// Posts to the loop's window from any thread.
#[derive(Debug, Clone, Copy)]
pub struct Waker(isize);

impl Waker {
    /// Wakes the loop with [`Event::Wake`].
    pub fn wake(self) {
        self.post(WM_WAKE, 0);
    }

    pub(crate) fn post(self, msg: u32, wparam: usize) {
        let hwnd = HWND(self.0 as *mut c_void);
        // SAFETY: PostMessageW may be called from any thread; a destroyed
        // window only makes the call fail.
        let _ = unsafe { PostMessageW(Some(hwnd), msg, WPARAM(wparam), LPARAM(0)) };
    }
}

/// The hidden window and its message loop. Create one per tray process, on
/// the thread that will run it.
#[derive(Debug)]
pub struct EventLoop {
    hwnd: HWND,
}

impl EventLoop {
    /// Creates the hidden window.
    ///
    /// # Errors
    /// Windows errors from registering the class or creating the window.
    pub fn new() -> Result<Self> {
        // SAFETY: a null module name returns this executable's handle.
        let instance = unsafe { GetModuleHandleW(None) }?;
        let class = WNDCLASSEXW {
            cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised and its strings are static.
        if unsafe { RegisterClassExW(&class) } == 0 {
            return Err(Error::from_thread());
        }
        // SAFETY: registers a static string.
        TASKBAR_CREATED.set(unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) });
        // SAFETY: the class was registered above; the window is a hidden,
        // top-level pop-up so it receives broadcasts such as WM_DISPLAYCHANGE.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS_NAME,
                w!("Huedra tray"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }?;
        WINDOW.set(hwnd.0 as isize);
        Ok(Self { hwnd })
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// A handle other threads can use to wake the loop.
    pub fn waker(&self) -> Waker {
        Waker(self.hwnd.0 as isize)
    }

    /// Starts or restarts a timer. It repeats until [`Self::kill_timer`].
    pub fn set_timer(&self, id: TimerId, after: Duration) {
        let ms = u32::try_from(after.as_millis())
            .unwrap_or(u32::MAX)
            .max(MIN_TIMER_MS);
        // SAFETY: the window belongs to this thread; a timer with the same id
        // is replaced.
        unsafe { SetTimer(Some(self.hwnd), id.0, ms, None) };
    }

    /// Stops a timer.
    pub fn kill_timer(&self, id: TimerId) {
        // SAFETY: stopping a timer that does not exist only fails.
        let _ = unsafe { KillTimer(Some(self.hwnd), id.0) };
    }

    /// Runs until the handler returns [`Flow::Exit`] or `WM_QUIT` arrives.
    ///
    /// # Errors
    /// A failure of `GetMessageW`.
    pub fn run(&mut self, mut handler: impl FnMut(&mut Self, Event) -> Flow) -> Result<()> {
        let mut msg = MSG::default();
        loop {
            // SAFETY: `msg` is a valid out-parameter.
            let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            match got.0 {
                0 => return Ok(()),
                -1 => return Err(Error::from_thread()),
                _ => {}
            }
            // SAFETY: `msg` was filled in by GetMessageW.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            while let Some(event) = pop() {
                if handler(self, event) == Flow::Exit {
                    return Ok(());
                }
            }
        }
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        WINDOW.set(0);
        // SAFETY: the window was created by this thread and is destroyed once.
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let event = match msg {
        WM_DISPLAYCHANGE => Some(Event::DisplayChanged),
        WM_HOTKEY => Hotkey::from_id(wparam.0).map(Event::Hotkey),
        WM_TIMER => Some(Event::Timer(TimerId(wparam.0))),
        WM_TRAY => match u32::try_from(lparam.0 & 0xFFFF).unwrap_or(0) {
            WM_CONTEXTMENU => Some(Event::TrayMenu),
            NIN_SELECT | NIN_KEYSELECT => Some(Event::TrayClick),
            _ => None,
        },
        WM_PROCESS_EXIT => u32::try_from(wparam.0).ok().map(Event::ProcessExited),
        WM_WAKE => Some(Event::Wake),
        m if m != 0 && m == TASKBAR_CREATED.get() => Some(Event::TaskbarCreated),
        _ => None,
    };
    match event {
        Some(event) => {
            // Queue directly: posting another wake from inside the window
            // procedure is unnecessary, the loop drains after dispatching.
            QUEUE.with_borrow_mut(|q| q.push_back(event));
            LRESULT(0)
        }
        // SAFETY: unhandled messages go to the default procedure unchanged.
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
