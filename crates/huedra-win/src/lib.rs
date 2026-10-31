//! Every call Huedra makes into Windows, and all of its `unsafe` code.
//!
//! Each `unsafe` block carries a `// SAFETY:` comment. The other crates
//! forbid `unsafe` entirely. On other platforms this crate is empty, so the
//! workspace still builds and tests on Linux.

#[cfg(windows)]
pub mod clock;
#[cfg(windows)]
pub mod console;
#[cfg(windows)]
pub mod display;
#[cfg(windows)]
pub mod event_loop;
#[cfg(windows)]
pub mod foreground;
#[cfg(windows)]
mod handle;
#[cfg(windows)]
pub mod hotkey;
#[cfg(windows)]
pub mod http;
#[cfg(windows)]
pub mod instance;
#[cfg(windows)]
pub mod menu;
#[cfg(windows)]
pub mod pipe;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod registry;
#[cfg(windows)]
pub mod shell;
#[cfg(windows)]
pub mod stores;
#[cfg(windows)]
pub mod tray;
