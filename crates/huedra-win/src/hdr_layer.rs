//! An HDR layer on a window: a DirectComposition visual with
//! an FP16 surface, which Windows composes as scRGB, so it can show colors
//! past sRGB and brighter than SDR white on a display in HDR mode. Asked
//! to go behind the window's drawing, it still shows over it (seen on
//! Windows 11 with a Slint window), so the window keeps that part free. Feature doc: `docs/features/08-settings-window.md`.

use windows::Win32::Foundation::{HMODULE, HWND, POINT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionRectangleClip,
    IDCompositionSurface, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_R16G16B16A16_FLOAT,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::Interface as _;

/// Why the HDR layer could not be made or changed.
#[derive(Debug, thiserror::Error)]
#[error("the HDR layer failed: {0}")]
pub struct HdrLayerError(#[from] windows::core::Error);

/// A rectangle in the window's client area, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Where the layer goes: `rect` with its top and bottom corners rounded
/// by the radii, showing only the part inside `view`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// The layer's rectangle.
    pub rect: PixelRect,
    /// Radius of the top corners.
    pub top_radius: f32,
    /// Radius of the bottom corners.
    pub bottom_radius: f32,
    /// The part of the window it may show in.
    pub view: PixelRect,
}

/// The layer: one visual over the window's drawing, with its content and
/// a clip with rounded corners.
pub struct HdrLayer {
    _d3d: ID3D11Device,
    context: ID3D11DeviceContext,
    device: IDCompositionDevice,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual,
    clip: IDCompositionRectangleClip,
    surface: Option<IDCompositionSurface>,
}

impl HdrLayer {
    /// Puts an empty layer on `hwnd`, the raw handle of a window of this
    /// process.
    ///
    /// # Errors
    /// Direct3D or DirectComposition refused (no graphics device).
    pub fn new(hwnd: isize) -> Result<Self, HdrLayerError> {
        let mut d3d = None;
        let mut context = None;
        // SAFETY: plain out parameters for a hardware device with no adapter
        // chosen; both are written before the call returns successfully.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&raw mut d3d),
                None,
                Some(&raw mut context),
            )?;
        }
        let d3d: ID3D11Device = d3d.ok_or_else(windows::core::Error::empty)?;
        let context = context.ok_or_else(windows::core::Error::empty)?;
        let dxgi: IDXGIDevice = d3d.cast()?;
        // SAFETY: `dxgi` is a live DXGI device for the call.
        let device: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi)? };
        // SAFETY: `hwnd` is a window of this process (the caller's promise).
        // `false` asks for the visuals behind the window's child windows.
        let target = unsafe { device.CreateTargetForHwnd(HWND(hwnd as _), false)? };
        // SAFETY: COM calls on the device and visuals just created.
        let (visual, clip) = unsafe {
            let visual = device.CreateVisual()?;
            let clip = device.CreateRectangleClip()?;
            visual.SetClip(&clip)?;
            target.SetRoot(&visual)?;
            device.Commit()?;
            (visual, clip)
        };
        Ok(Self {
            _d3d: d3d,
            context,
            device,
            _target: target,
            visual,
            clip,
            surface: None,
        })
    }

    /// Shows `pixels`: RGBA as the bits of 16-bit floats, premultiplied,
    /// scRGB, `width` x `height`.
    ///
    /// # Errors
    /// DirectComposition refused the surface or the drawing, or `pixels` is
    /// shorter than the size says.
    pub fn set_image(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u16],
    ) -> Result<(), HdrLayerError> {
        if width == 0 || height == 0 || pixels.len() < (width * height * 4) as usize {
            return Err(windows::core::Error::empty().into());
        }
        // SAFETY: COM calls on live objects. `BeginDraw` hands out the
        // surface's texture and where in it to draw; the copy reads
        // `width * height * 4` halves from `pixels`, checked above, and
        // `EndDraw` follows the drawing.
        unsafe {
            let surface = self.device.CreateSurface(
                width,
                height,
                DXGI_FORMAT_R16G16B16A16_FLOAT,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            )?;
            let mut offset = POINT::default();
            let texture: ID3D11Texture2D = surface.BeginDraw(None, &raw mut offset)?;
            #[expect(clippy::cast_sign_loss, reason = "the offset is never negative")]
            let (left, top) = (offset.x as u32, offset.y as u32);
            let area = D3D11_BOX {
                left,
                top,
                front: 0,
                right: left + width,
                bottom: top + height,
                back: 1,
            };
            self.context.UpdateSubresource(
                &texture,
                0,
                Some(&raw const area),
                pixels.as_ptr().cast(),
                width * 8,
                0,
            );
            surface.EndDraw()?;
            self.visual.SetContent(&surface)?;
            self.device.Commit()?;
            self.surface = Some(surface);
        }
        Ok(())
    }

    /// Puts the layer where `placement` says; `None` hides it.
    ///
    /// # Errors
    /// DirectComposition refused the change.
    pub fn place(&self, placement: Option<Placement>) -> Result<(), HdrLayerError> {
        // SAFETY: COM calls on live objects with plain numbers.
        unsafe {
            match placement {
                Some(Placement {
                    rect: card,
                    top_radius,
                    bottom_radius,
                    view,
                }) => {
                    self.visual.SetOffsetX2(card.x)?;
                    self.visual.SetOffsetY2(card.y)?;
                    // The clip is in the card's own coordinates.
                    let left = (view.x - card.x).max(0.0);
                    let top = (view.y - card.y).max(0.0);
                    let right = (view.x + view.width - card.x).min(card.width);
                    let bottom = (view.y + view.height - card.y).min(card.height);
                    self.clip.SetLeft2(left)?;
                    self.clip.SetTop2(top)?;
                    self.clip.SetRight2(right.max(left))?;
                    self.clip.SetBottom2(bottom.max(top))?;
                    self.clip.SetTopLeftRadiusX2(top_radius)?;
                    self.clip.SetTopLeftRadiusY2(top_radius)?;
                    self.clip.SetTopRightRadiusX2(top_radius)?;
                    self.clip.SetTopRightRadiusY2(top_radius)?;
                    self.clip.SetBottomLeftRadiusX2(bottom_radius)?;
                    self.clip.SetBottomLeftRadiusY2(bottom_radius)?;
                    self.clip.SetBottomRightRadiusX2(bottom_radius)?;
                    self.clip.SetBottomRightRadiusY2(bottom_radius)?;
                }
                None => {
                    self.clip.SetLeft2(0.0)?;
                    self.clip.SetTop2(0.0)?;
                    self.clip.SetRight2(0.0)?;
                    self.clip.SetBottom2(0.0)?;
                }
            }
            self.device.Commit()?;
        }
        Ok(())
    }
}
