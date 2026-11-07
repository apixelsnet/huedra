//! The notification-area icon, written directly against `Shell_NotifyIconW`.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

use huedra_core::icon::{self, TrayIcon};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HBITMAP,
};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_RESPECT_QUIET_TIME, NIIF_USER,
    NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
    NOTIFYICONIDENTIFIER, Shell_NotifyIconGetRect, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, HICON, ICONINFO, SM_CXSMICON,
};
use windows::core::Result;

use crate::event_loop::{EventLoop, WM_TRAY};

const ICON_ID: u32 = 1;
const FALLBACK_SIZE: u32 = 16;

/// The tray icon. Removed from the notification area on drop.
#[derive(Debug)]
pub struct Tray {
    hwnd: HWND,
    hdr: HICON,
    sdr: HICON,
    shown: Option<(TrayIcon, String)>,
    added: bool,
}

impl Tray {
    /// Prepares both icons at the system's small-icon size. Nothing is shown
    /// until [`Self::show`].
    ///
    /// # Errors
    /// Windows errors from creating the icons.
    pub fn new(host: &EventLoop) -> Result<Self> {
        // SAFETY: plain metric queries.
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
        let size = u32::try_from(size)
            .ok()
            .filter(|&s| s > 0)
            .unwrap_or(FALLBACK_SIZE);
        Ok(Self {
            hwnd: host.hwnd(),
            hdr: create_icon(TrayIcon::Hdr, size)?,
            sdr: create_icon(TrayIcon::Sdr, size)?,
            shown: None,
            added: false,
        })
    }

    /// Shows `icon` with `tip`; does nothing when both are unchanged.
    pub fn show(&mut self, icon: TrayIcon, tip: &str) {
        if self.added
            && self
                .shown
                .as_ref()
                .is_some_and(|(i, t)| *i == icon && t == tip)
        {
            return;
        }
        let mut data = self.data(icon, tip);
        if self.added {
            // SAFETY: `data` is fully initialized for this window and id.
            let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
        } else {
            // SAFETY: as above. Fails while Explorer is not running; the icon
            // is then added on TaskbarCreated.
            self.added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
            if self.added {
                data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                // SAFETY: as above.
                let _ = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) };
            }
        }
        self.shown = Some((icon, tip.to_owned()));
    }

    /// Shows a Windows notification from the icon. Windows holds it while
    /// quiet hours or a full-screen game ask for quiet, and keeps it in the
    /// notification center.
    pub fn notify(&self, title: &str, text: &str) {
        let Some((icon, tip)) = self.shown.as_ref().filter(|_| self.added) else {
            return;
        };
        let mut data = self.data(*icon, tip);
        data.uFlags |= NIF_INFO;
        data.dwInfoFlags = NIIF_USER | NIIF_RESPECT_QUIET_TIME;
        copy_text(&mut data.szInfoTitle, title);
        copy_text(&mut data.szInfo, text);
        // SAFETY: `data` is fully initialized for this window and id.
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }

    /// Where the icon is on screen, in physical pixels: on the taskbar, or
    /// in the overflow area while that is open. `None` while it is not
    /// shown.
    pub fn rect(&self) -> Option<huedra_core::flyout::Rect> {
        if !self.added {
            return None;
        }
        let id = NOTIFYICONIDENTIFIER {
            cbSize: u32::try_from(size_of::<NOTIFYICONIDENTIFIER>()).unwrap_or(0),
            hWnd: self.hwnd,
            uID: ICON_ID,
            ..Default::default()
        };
        // SAFETY: `id` names this object's icon and outlives the call.
        let r = unsafe { Shell_NotifyIconGetRect(&id) }.ok()?;
        Some(huedra_core::flyout::Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        })
    }

    /// Adds the icon again after Explorer restarted.
    pub fn restore(&mut self) {
        self.added = false;
        if let Some((icon, tip)) = self.shown.take() {
            self.show(icon, &tip);
        }
    }

    fn data(&self, icon: TrayIcon, tip: &str) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: u32::try_from(size_of::<NOTIFYICONDATAW>()).unwrap_or(0),
            hWnd: self.hwnd,
            uID: ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_TRAY,
            hIcon: match icon {
                TrayIcon::Hdr => self.hdr,
                TrayIcon::Sdr => self.sdr,
            },
            ..Default::default()
        };
        copy_text(&mut data.szTip, tip);
        data
    }
}

/// Copies `text` into a fixed buffer, cut to leave room for the closing zero.
fn copy_text(buffer: &mut [u16], text: &str) {
    let room = buffer.len().saturating_sub(1);
    for (slot, unit) in buffer.iter_mut().zip(text.encode_utf16().take(room)) {
        *slot = unit;
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if self.added {
            let data = NOTIFYICONDATAW {
                cbSize: u32::try_from(size_of::<NOTIFYICONDATAW>()).unwrap_or(0),
                hWnd: self.hwnd,
                uID: ICON_ID,
                ..Default::default()
            };
            // SAFETY: removes the icon this object added.
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
        }
        // SAFETY: both icons were created by `create_icon` and are destroyed once.
        unsafe {
            let _ = DestroyIcon(self.hdr);
            let _ = DestroyIcon(self.sdr);
        }
    }
}

/// A GDI bitmap deleted on drop.
struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: created by this module and deleted once.
        let _ = unsafe { DeleteObject(self.0.into()) };
    }
}

fn create_icon(kind: TrayIcon, size: u32) -> Result<HICON> {
    let rgba = icon::render(kind, size);
    let side = i32::try_from(size).unwrap_or(16);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).unwrap_or(0),
            biWidth: side,
            biHeight: -side, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = ptr::null_mut();
    // SAFETY: describes a 32-bit top-down bitmap; Windows allocates the pixels
    // and returns them in `bits`.
    let color =
        Bitmap(unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }?);
    if bits.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    // SAFETY: `bits` points to size * size * 4 bytes owned by `color`.
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), rgba.len()) };
    for (dst, src) in pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(rgba.as_chunks::<4>().0)
    {
        *dst = [src[2], src[1], src[0], src[3]];
    }
    // A 1-bit mask of zeros, rows padded to 16 bits; the alpha channel decides.
    let mask_bits = vec![0_u8; size.div_ceil(16) as usize * 2 * size as usize];
    // SAFETY: `mask_bits` holds a full 1-bit image of this size.
    let mask = Bitmap(unsafe {
        CreateBitmap(side, side, 1, 1, Some(mask_bits.as_ptr().cast::<c_void>()))
    });
    let icon_info = ICONINFO {
        fIcon: true.into(),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask.0,
        hbmColor: color.0,
    };
    // SAFETY: both bitmaps are valid; Windows copies them into the icon.
    unsafe { CreateIconIndirect(&icon_info) }
}
