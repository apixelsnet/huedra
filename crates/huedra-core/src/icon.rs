//! Draws the tray icons: a full-spectrum dot while HDR is on, a grey ring in
//! SDR. Pixels are computed here so the shapes are tested on any platform.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

/// Which icon to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIcon {
    /// HDR is on.
    Hdr,
    /// SDR.
    Sdr,
}

/// The BT.2020-derived palette, in order around the dot.
const SPECTRUM: [[u8; 3]; 5] = [
    [0xFF, 0x2D, 0x6F], // pink
    [0x9B, 0x3D, 0xFF], // violet
    [0x33, 0x41, 0xE6], // ultramarine
    [0x00, 0xC8, 0xF0], // cyan
    [0x14, 0xD9, 0x8A], // green
];
const SDR_GREY: [u8; 3] = [0xA9, 0xA4, 0xB6];
/// Samples per pixel along each axis, for smooth edges.
const SUPERSAMPLE: u32 = 4;

/// Renders an icon of `size` x `size` pixels as straight (not premultiplied)
/// RGBA, rows top to bottom.
#[expect(
    clippy::cast_precision_loss,
    reason = "icon sizes are below 256 pixels"
)]
pub fn render(icon: TrayIcon, size: u32) -> Vec<u8> {
    let n = size as f32;
    let center = n / 2.0;
    let outer = n * 0.42;
    let inner = match icon {
        TrayIcon::Hdr => 0.0,
        TrayIcon::Sdr => outer - (n * 0.13).max(1.6),
    };
    let samples = SUPERSAMPLE * SUPERSAMPLE;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut covered = 0_u32;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let px = x as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32 - center;
                    let py = y as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32 - center;
                    let d = (px * px + py * py).sqrt();
                    if d <= outer && d >= inner {
                        covered += 1;
                    }
                }
            }
            let rgb = match icon {
                TrayIcon::Hdr => spectrum_at(x as f32 + 0.5 - center, y as f32 + 0.5 - center),
                TrayIcon::Sdr => SDR_GREY,
            };
            pixels.extend_from_slice(&rgb);
            pixels.push(u8::try_from(covered * 255 / samples).unwrap_or(u8::MAX));
        }
    }
    pixels
}

/// Renders the spectrum as a `width` x `height` RGBA strip running across at
/// an angle, for backgrounds. Drawn here rather than by the UI renderer so it
/// is free of banding and seams.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "image sizes are small; values are bounded to 0..=255"
)]
pub fn spectrum_strip(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let last = (SPECTRUM.len() - 1) as f32;
    for y in 0..height {
        for x in 0..width {
            // Position along a slanted axis, mapped to 0..1.
            let t = ((x as f32 / w) * 0.85 + (y as f32 / h) * 0.15).clamp(0.0, 1.0);
            let scaled = t * last;
            let index = (scaled.floor() as usize).min(SPECTRUM.len() - 2);
            let f = scaled - index as f32;
            let (a, b) = (SPECTRUM[index], SPECTRUM[index + 1]);
            let mix =
                |p: u8, q: u8| (f32::from(p) + (f32::from(q) - f32::from(p)) * f).round() as u8;
            pixels.extend_from_slice(&[mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), 255]);
        }
    }
    pixels
}

/// Colour of the conic gradient at an offset from the centre, starting at
/// the top and running clockwise.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "values are bounded to 0..=255 and 0..5 by construction"
)]
fn spectrum_at(dx: f32, dy: f32) -> [u8; 3] {
    let turn = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
    let scaled = turn * SPECTRUM.len() as f32;
    let index = (scaled.floor() as usize) % SPECTRUM.len();
    let next = (index + 1) % SPECTRUM.len();
    let t = scaled.fract();
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    let (a, b) = (SPECTRUM[index], SPECTRUM[next]);
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(pixels: &[u8], size: u32, x: u32, y: u32) -> u8 {
        pixels[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn dot_and_ring_shapes() {
        let size = 32;
        let dot = render(TrayIcon::Hdr, size);
        let ring = render(TrayIcon::Sdr, size);
        assert_eq!(dot.len(), (size * size * 4) as usize, "buffer size");
        assert_eq!(alpha(&dot, size, 16, 16), 255, "dot is filled");
        assert_eq!(alpha(&ring, size, 16, 16), 0, "ring is hollow");
        assert_eq!(alpha(&ring, size, 16, 3), 255, "ring edge is drawn");
        assert_eq!(alpha(&dot, size, 0, 0), 0, "corners are clear");
    }

    #[test]
    fn strip_runs_pink_to_green() {
        let strip = spectrum_strip(100, 10);
        assert_eq!(strip.len(), 100 * 10 * 4, "size");
        assert_eq!(&strip[..3], &SPECTRUM[0], "starts pink");
        let end = (9 * 100 + 99) * 4;
        assert!(strip[end + 1] > 0xB0, "ends near green");
    }

    #[test]
    fn spectrum_starts_pink_and_has_no_yellow() {
        assert_eq!(spectrum_at(0.0, -1.0), SPECTRUM[0], "top is pink");
        for i in 0..360_u16 {
            let a = f32::from(i).to_radians();
            let [r, g, b] = spectrum_at(a.sin(), -a.cos());
            let yellow = r > 180 && g > 180 && b < 90;
            assert!(!yellow, "no yellow at {i} degrees: {r},{g},{b}");
        }
    }
}
