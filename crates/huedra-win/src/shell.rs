//! Small shell services for the settings window: opening a web page, the
//! file picker, a program's icon, desktop shortcuts, and the Windows
//! "Animation effects" setting.

use std::mem::size_of;
use std::path::{Path, PathBuf};

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
};
use windows::core::Interface as _;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, HBITMAP, HDC,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

/// Opens an `https://` address in the default browser. Other schemes are
/// refused, so nothing else can be started this way.
pub fn open_web_page(url: &str) -> bool {
    if !url.starts_with("https://") {
        return false;
    }
    // SAFETY: opens a URL with the default handler; strings outlive the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(url),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    result.0 as usize > 32
}

/// Asks the user for a program (`.exe`). Returns its path.
pub fn pick_program(title: &str) -> Option<String> {
    let mut file = vec![0_u16; 4096];
    let filter: Vec<u16> = "Programs (*.exe)\0*.exe\0"
        .encode_utf16()
        .chain([0])
        .collect();
    let title = HSTRING::from(title);
    let mut dialog = OPENFILENAMEW {
        lStructSize: u32::try_from(size_of::<OPENFILENAMEW>()).unwrap_or(0),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: u32::try_from(file.len()).unwrap_or(0),
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: every buffer referenced by `dialog` outlives the modal call.
    if !unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(String::from_utf16_lossy(&file[..len]))
}

/// The current user's desktop folder.
pub fn desktop_folder() -> Option<PathBuf> {
    // SAFETY: asks for a known folder; the returned string is freed below.
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: a null-terminated string from the shell, read before it is freed.
    let text = unsafe { path.to_string() }.ok();
    // SAFETY: allocated by the shell with CoTaskMemAlloc and freed once.
    unsafe { CoTaskMemFree(Some(path.0.cast_const().cast())) };
    text.map(PathBuf::from)
}

/// A shortcut (`.lnk`) to write.
#[derive(Debug, Clone, Copy)]
pub struct Shortcut<'a> {
    /// The program it starts.
    pub target: &'a str,
    /// Its arguments.
    pub arguments: &'a str,
    /// The folder it starts in.
    pub working_dir: &'a str,
    /// The file whose first icon it shows.
    pub icon: &'a str,
    /// Tooltip text.
    pub description: &'a str,
}

/// Writes a shortcut file, replacing one of the same name.
///
/// # Errors
/// The COM error from the shell.
pub fn create_shortcut(file: &Path, shortcut: &Shortcut<'_>) -> windows::core::Result<()> {
    let _com = init_com();
    // SAFETY: creates the shell's link object in this process.
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }?;
    // SAFETY: each call copies the string it is given.
    unsafe {
        link.SetPath(&HSTRING::from(shortcut.target))?;
        link.SetArguments(&HSTRING::from(shortcut.arguments))?;
        link.SetWorkingDirectory(&HSTRING::from(shortcut.working_dir))?;
        link.SetIconLocation(&HSTRING::from(shortcut.icon), 0)?;
        link.SetDescription(&HSTRING::from(shortcut.description))?;
    }
    let persist: IPersistFile = link.cast()?;
    // SAFETY: writes the link to `file`; the path string outlives the call.
    unsafe { persist.Save(&HSTRING::from(file.as_os_str()), true) }
}

/// Initializes COM on this thread for the guard's lifetime. A thread
/// already initialized (S_FALSE, or another apartment type) keeps working;
/// only a successful call is balanced by the guard.
pub(crate) fn init_com() -> Option<ComGuard> {
    // SAFETY: initializes COM on this thread; balanced by the guard.
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    init.is_ok().then_some(ComGuard)
}

/// Balances a successful `CoInitializeEx`.
pub(crate) struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: matches the successful CoInitializeEx on this thread.
        unsafe { CoUninitialize() };
    }
}

/// Whether Windows "Animation effects" are on. Defaults to on if unknown.
pub fn animations_enabled() -> bool {
    let mut enabled = windows::core::BOOL(1);
    // SAFETY: `enabled` is a BOOL, as this action requires.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut enabled).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_err() || enabled.as_bool()
}

/// The icon inside a program, `size` x `size` pixels, as straight RGBA rows
/// from the top. `None` when the file has no icon or cannot be read.
pub fn program_icon(path: &str, size: u32) -> Option<Vec<u8>> {
    let side = i32::try_from(size).ok()?;
    let mut icon = HICON::default();
    // SAFETY: asks the shell for the program's first icon at `size` pixels
    // (large size in the low word); `icon` receives a handle we destroy.
    let result = unsafe {
        SHDefExtractIconW(
            &HSTRING::from(path),
            0,
            0,
            Some(&raw mut icon),
            None,
            size & 0xFFFF,
        )
    };
    if result.is_err() || icon.is_invalid() {
        return None;
    }
    let icon = OwnedIcon(icon);
    let mut info = ICONINFO::default();
    // SAFETY: `icon.0` is a valid icon; Windows fills `info` and hands us
    // two bitmaps, which the guards below delete.
    unsafe { GetIconInfo(icon.0, &raw mut info) }.ok()?;
    let color = OwnedBitmap(info.hbmColor);
    let mask = OwnedBitmap(info.hbmMask);
    if color.0.is_invalid() {
        return None;
    }
    // SAFETY: a memory device context compatible with the screen, deleted by
    // the guard.
    let dc = OwnedDc(unsafe { CreateCompatibleDC(None) });
    let mut pixels = read_bitmap(dc.0, color.0, side)?;
    // Icons without an alpha channel use the mask for transparency.
    if pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
        let mask_pixels = read_bitmap(dc.0, mask.0, side)?;
        let masks = mask_pixels.as_chunks::<4>().0;
        for (pixel, m) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(masks) {
            pixel[3] = if m[0] == 0 { 255 } else { 0 };
        }
    }
    // Windows gives blue, green, red; swap to red, green, blue.
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Some(pixels)
}

/// A bitmap as 32-bit rows from the top, `side` x `side` pixels.
fn read_bitmap(dc: HDC, bitmap: HBITMAP, side: i32) -> Option<Vec<u8>> {
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).ok()?,
            biWidth: side,
            biHeight: -side,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let bytes = usize::try_from(side).ok()?.pow(2) * 4;
    let mut pixels = vec![0_u8; bytes];
    // SAFETY: `pixels` holds `side` rows of `side` 32-bit pixels, exactly
    // what the header asks Windows to write.
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            side.unsigned_abs(),
            Some(pixels.as_mut_ptr().cast()),
            &raw mut header,
            DIB_RGB_COLORS,
        )
    };
    (lines == side).then_some(pixels)
}

struct OwnedIcon(HICON);

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        // SAFETY: an icon handed to us by the shell, destroyed once.
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

struct OwnedBitmap(HBITMAP);

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: a bitmap from `GetIconInfo`, which the caller must delete.
            let _ = unsafe { DeleteObject(self.0.into()) };
        }
    }
}

struct OwnedDc(HDC);

impl Drop for OwnedDc {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: a device context we created, deleted once.
            let _ = unsafe { DeleteDC(self.0) };
        }
    }
}
