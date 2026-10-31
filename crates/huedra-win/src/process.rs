//! Program paths and exit notifications. Only the rights to read a
//! program's path and to wait for it are requested; process memory is never
//! touched, which keeps anti-cheat software unaffected.
//! Feature doc: `docs/features/03-watchers.md`.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    INFINITE, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, RegisterWaitForSingleObject, UnregisterWaitEx,
    WT_EXECUTEONLYONCE,
};
use windows::core::PWSTR;

use crate::event_loop::{WM_PROCESS_EXIT, Waker};
use crate::handle::OwnedHandle;

/// Longest path `QueryFullProcessImageNameW` can return.
const MAX_PATH_CHARS: u32 = 32_768;

/// The full path of a running program, if Windows allows reading it.
pub fn image_path(pid: u32) -> Option<String> {
    // SAFETY: requests only the limited query right.
    let process =
        OwnedHandle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?);
    path_of(process.0)
}

fn path_of(process: HANDLE) -> Option<String> {
    let mut buffer = vec![0_u16; MAX_PATH_CHARS as usize];
    let mut len = MAX_PATH_CHARS;
    // SAFETY: `buffer` holds `len` characters; Windows writes at most that
    // many and stores the count in `len`.
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
    }
    .ok()?;
    buffer.truncate(len as usize);
    Some(String::from_utf16_lossy(&buffer))
}

/// The ids of all running processes. Used only while following a launcher.
pub fn pids() -> Vec<u32> {
    let mut found = Vec::new();
    // SAFETY: takes a snapshot of the process list only.
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return found;
    };
    let snapshot = OwnedHandle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `entry` has its size set, as the function requires.
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
    while more {
        found.push(entry.th32ProcessID);
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
    }
    found
}

struct WaitContext {
    waker: Waker,
    pid: u32,
}

/// Reports [`crate::event_loop::Event::ProcessExited`] when a process ends,
/// without using processor time while it runs. Dropping it cancels the wait.
pub struct ExitWatch {
    wait: HANDLE,
    context: *mut WaitContext,
    _process: OwnedHandle,
}

impl ExitWatch {
    /// Starts waiting for `pid` to exit.
    pub fn new(pid: u32, waker: Waker) -> Option<Self> {
        // SAFETY: requests only the rights to wait for and name the process.
        let process = OwnedHandle(
            unsafe {
                OpenProcess(
                    PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    pid,
                )
            }
            .ok()?,
        );
        let context = Box::into_raw(Box::new(WaitContext { waker, pid }));
        let mut wait = HANDLE::default();
        // SAFETY: `context` stays valid until Drop, which unregisters the
        // wait and waits for a running callback before freeing it.
        let registered = unsafe {
            RegisterWaitForSingleObject(
                &mut wait,
                process.0,
                Some(on_exit),
                Some(context.cast_const().cast::<c_void>()),
                INFINITE,
                WT_EXECUTEONLYONCE,
            )
        };
        if registered.is_err() {
            // SAFETY: created just above and never shared.
            drop(unsafe { Box::from_raw(context) });
            return None;
        }
        Some(Self {
            wait,
            context,
            _process: process,
        })
    }
}

impl Drop for ExitWatch {
    fn drop(&mut self) {
        // SAFETY: INVALID_HANDLE_VALUE makes the call wait until a running
        // callback has finished, so the context can be freed afterwards.
        let _ = unsafe { UnregisterWaitEx(self.wait, Some(INVALID_HANDLE_VALUE)) };
        // SAFETY: created by Box::into_raw in `new`; no callback can run now.
        drop(unsafe { Box::from_raw(self.context) });
    }
}

unsafe extern "system" fn on_exit(context: *mut c_void, _timed_out: bool) {
    // SAFETY: `context` is the WaitContext registered in `ExitWatch::new`,
    // alive until the wait is unregistered.
    let context = unsafe { &*context.cast::<WaitContext>() };
    context.waker.post(WM_PROCESS_EXIT, context.pid as usize);
}
