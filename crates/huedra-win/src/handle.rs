//! Drop guards for Windows handles.

use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};

/// A kernel handle closed on drop.
#[derive(Debug)]
pub(crate) struct OwnedHandle(pub(crate) HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: the handle was opened by this process and is closed once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

/// A kernel handle that may move to another thread. Kernel handles are not
/// tied to the thread that opened them.
#[derive(Debug)]
pub(crate) struct SendHandle(pub(crate) OwnedHandle);

// SAFETY: kernel object handles are process-wide and may be used from any
// thread; only one thread uses this one at a time because it is owned.
unsafe impl Send for SendHandle {}

/// Memory from `LocalAlloc`, freed on drop.
pub(crate) struct LocalMemory(pub(crate) *mut core::ffi::c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer was returned by a function documented to
            // allocate with LocalAlloc, and is freed once.
            unsafe { LocalFree(Some(HLOCAL(self.0))) };
        }
    }
}
