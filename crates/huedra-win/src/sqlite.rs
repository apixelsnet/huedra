//! Read-only queries on the SQLite databases of the Amazon Games and itch.io
//! apps, through `winsqlite3.dll`, the SQLite that is part of Windows 10 and
//! 11. It is loaded from System32 only, so no copy beside the program can
//! stand in for it, and only when one of those databases exists.
//! Feature doc: `docs/features/04-game-library.md`.

use std::ffi::{CString, c_char, c_int, c_void};
use std::path::Path;

use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::core::{PCSTR, w};

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_OPEN_READONLY: c_int = 0x0000_0001;
/// How long a query waits while the app itself writes, in milliseconds.
const BUSY_TIMEOUT_MS: c_int = 500;
/// More rows than any game library has.
const MAX_ROWS: usize = 4096;
/// Longer text in a cell is cut (the itch.io verdicts are a few KB).
const MAX_CELL_BYTES: usize = 256 * 1024;

type Open =
    unsafe extern "system" fn(*const c_char, *mut *mut c_void, c_int, *const c_char) -> c_int;
type Close = unsafe extern "system" fn(*mut c_void) -> c_int;
type BusyTimeout = unsafe extern "system" fn(*mut c_void, c_int) -> c_int;
type Prepare = unsafe extern "system" fn(
    *mut c_void,
    *const c_char,
    c_int,
    *mut *mut c_void,
    *mut *const c_char,
) -> c_int;
type Step = unsafe extern "system" fn(*mut c_void) -> c_int;
type ColumnText = unsafe extern "system" fn(*mut c_void, c_int) -> *const u8;
type ColumnBytes = unsafe extern "system" fn(*mut c_void, c_int) -> c_int;

/// A loaded DLL, freed on drop.
struct Module(HMODULE);

impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: loaded by `Sqlite::load` and freed once, after every
        // database and statement opened through it was closed (they borrow
        // the `Sqlite` that owns this).
        let _ = unsafe { FreeLibrary(self.0) };
    }
}

/// The functions used. `module` is the last field, so it is freed last.
struct Sqlite {
    open: Open,
    close: Close,
    busy_timeout: BusyTimeout,
    prepare: Prepare,
    step: Step,
    finalize: Step,
    column_text: ColumnText,
    column_bytes: ColumnBytes,
    #[expect(dead_code, reason = "held only to keep the DLL loaded")]
    module: Module,
}

/// Looks up one export and gives it the function type `T`.
///
/// # Safety
/// `T` must be the export's real signature, as a function pointer type.
unsafe fn export<T: Copy>(module: HMODULE, name: &[u8]) -> Option<T> {
    debug_assert!(name.ends_with(b"\0"), "null-terminated name");
    // SAFETY: `name` is a null-terminated ASCII string and `module` is
    // loaded.
    let found = unsafe { GetProcAddress(module, PCSTR(name.as_ptr())) }?;
    debug_assert_eq!(size_of::<T>(), size_of_val(&found), "function pointer");
    // SAFETY: both are function pointers of the same size; the caller
    // vouches for the signature.
    Some(unsafe { std::mem::transmute_copy::<unsafe extern "system" fn() -> isize, T>(&found) })
}

impl Sqlite {
    fn load() -> Option<Self> {
        // SAFETY: loads a DLL that is part of Windows, from System32 only.
        let module =
            unsafe { LoadLibraryExW(w!("winsqlite3.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
                .ok()?;
        let module = Module(module);
        let m = module.0;
        // SAFETY: the types above are SQLite's documented C signatures; on
        // x64 `system` is the C calling convention. When one is missing,
        // `module` is dropped and the DLL freed.
        unsafe {
            Some(Self {
                open: export(m, b"sqlite3_open_v2\0")?,
                close: export(m, b"sqlite3_close\0")?,
                busy_timeout: export(m, b"sqlite3_busy_timeout\0")?,
                prepare: export(m, b"sqlite3_prepare_v2\0")?,
                step: export(m, b"sqlite3_step\0")?,
                finalize: export(m, b"sqlite3_finalize\0")?,
                column_text: export(m, b"sqlite3_column_text\0")?,
                column_bytes: export(m, b"sqlite3_column_bytes\0")?,
                module,
            })
        }
    }
}

/// An open database, closed on drop.
struct Db<'a> {
    sqlite: &'a Sqlite,
    handle: *mut c_void,
}

impl Drop for Db<'_> {
    fn drop(&mut self) {
        // SAFETY: opened by `sqlite3_open_v2` (which hands out a handle to
        // close even when opening failed); closed once, after its statement
        // was finalized.
        unsafe { (self.sqlite.close)(self.handle) };
    }
}

/// A prepared statement, finalized on drop.
struct Statement<'a> {
    sqlite: &'a Sqlite,
    handle: *mut c_void,
}

impl Drop for Statement<'_> {
    fn drop(&mut self) {
        // SAFETY: prepared by `sqlite3_prepare_v2` and finalized once.
        unsafe { (self.sqlite.finalize)(self.handle) };
    }
}

/// Runs `sql` on the database at `path`, opened read-only, and returns the
/// first `columns` columns of each row as text (an empty string for NULL).
/// Empty when the file, the DLL or the table is missing, or the query fails.
pub fn query(path: &Path, sql: &str, columns: usize) -> Vec<Vec<String>> {
    if !path.is_file() {
        return Vec::new();
    }
    let (Some(file), Ok(sql)) = (
        path.to_str().and_then(|p| CString::new(p).ok()),
        CString::new(sql),
    ) else {
        return Vec::new();
    };
    let Some(sqlite) = Sqlite::load() else {
        return Vec::new();
    };
    let mut handle = std::ptr::null_mut();
    // SAFETY: `file` is a null-terminated UTF-8 path; `handle` receives the
    // connection.
    let status = unsafe {
        (sqlite.open)(
            file.as_ptr(),
            &raw mut handle,
            SQLITE_OPEN_READONLY,
            std::ptr::null(),
        )
    };
    if handle.is_null() {
        return Vec::new();
    }
    let db = Db {
        sqlite: &sqlite,
        handle,
    };
    if status != SQLITE_OK {
        return Vec::new();
    }
    // SAFETY: `db` is open.
    unsafe { (sqlite.busy_timeout)(db.handle, BUSY_TIMEOUT_MS) };
    let mut handle = std::ptr::null_mut();
    // SAFETY: `sql` is null-terminated (-1 reads up to the null); `handle`
    // receives the statement.
    let status = unsafe {
        (sqlite.prepare)(
            db.handle,
            sql.as_ptr(),
            -1,
            &raw mut handle,
            std::ptr::null_mut(),
        )
    };
    if status != SQLITE_OK || handle.is_null() {
        return Vec::new();
    }
    let statement = Statement {
        sqlite: &sqlite,
        handle,
    };
    let mut rows = Vec::new();
    // SAFETY: the statement is prepared and used on this thread only.
    while rows.len() < MAX_ROWS && unsafe { (sqlite.step)(statement.handle) } == SQLITE_ROW {
        let row = (0..columns)
            .map(|column| {
                let column = c_int::try_from(column).unwrap_or(c_int::MAX);
                // SAFETY: the statement is on a row; the text pointer stays
                // valid until the next step and is copied before it.
                unsafe {
                    let text = (sqlite.column_text)(statement.handle, column);
                    let bytes = (sqlite.column_bytes)(statement.handle, column);
                    let len = usize::try_from(bytes).unwrap_or(0).min(MAX_CELL_BYTES);
                    if text.is_null() || len == 0 {
                        String::new()
                    } else {
                        String::from_utf8_lossy(std::slice::from_raw_parts(text, len)).into_owned()
                    }
                }
            })
            .collect();
        rows.push(row);
    }
    drop(statement);
    drop(db);
    rows
}
