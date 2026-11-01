//! Draws Huedra's icon: the outline of the colours the eye can see (the CIE
//! 1931 chromaticity diagram), filled with the BT.2020 palette while HDR is
//! on, with the BT.2020 triangle inside at larger sizes; a grey outline of
//! the same shape in SDR. Used for the tray, the program, the settings window
//! and the browser extension. Pixels are computed here so the shapes are
//! tested on any platform. Feature doc: `docs/features/07-tray-hotkeys.md`.

/// Which icon to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIcon {
    /// HDR is on.
    Hdr,
    /// SDR.
    Sdr,
}

/// The BT.2020-derived palette, in order around the shape, clockwise.
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
/// Where the colours start, in turns clockwise from the top: green sits at
/// the top, as on the diagram, and pink on the right.
const COLOUR_START: f32 = 0.2;

/// The spectral locus, 380 to 700 nm, as CIE 1931 x, y; the straight line
/// back to the start closes the shape.
const LOCUS: [[f32; 2]; 24] = [
    [0.1741, 0.0050],
    [0.1440, 0.0297],
    [0.1241, 0.0578],
    [0.1096, 0.0868],
    [0.0913, 0.1327],
    [0.0687, 0.2007],
    [0.0454, 0.2950],
    [0.0235, 0.4127],
    [0.0082, 0.5384],
    [0.0039, 0.6548],
    [0.0139, 0.7502],
    [0.0389, 0.8120],
    [0.0743, 0.8338],
    [0.1142, 0.8262],
    [0.1547, 0.8059],
    [0.2296, 0.7543],
    [0.3016, 0.6923],
    [0.3731, 0.6245],
    [0.4441, 0.5547],
    [0.5125, 0.4866],
    [0.5752, 0.4242],
    [0.6270, 0.3725],
    [0.6915, 0.3083],
    [0.7347, 0.2653],
];
/// BT.2020 red, green and blue, as CIE 1931 x, y.
const BT2020: [[f32; 2]; 3] = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]];
/// Smallest icon that shows the triangle.
const TRIANGLE_FROM: u32 = 32;

/// Maps diagram coordinates into an icon of `n` pixels, centred with a
/// small margin. The shape is drawn a quarter wider than the diagram's own
/// proportions, as charts usually show it.
fn place(point: [f32; 2], n: f32) -> [f32; 2] {
    const X0: f32 = 0.0039;
    const X1: f32 = 0.7347;
    const Y0: f32 = 0.0050;
    const Y1: f32 = 0.8338;
    let scale_y = n * 0.84 / (Y1 - Y0);
    let scale_x = scale_y * 1.25;
    let left = (n - (X1 - X0) * scale_x) / 2.0;
    let top = (n - (Y1 - Y0) * scale_y) / 2.0;
    [
        left + (point[0] - X0) * scale_x,
        top + (Y1 - point[1]) * scale_y,
    ]
}

/// Whether `p` lies inside the polygon (even-odd rule).
fn inside(polygon: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut result = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            result = !result;
        }
        j = i;
    }
    result
}

/// Distance from `p` to the nearest edge of the closed polygon.
fn edge_distance(polygon: &[[f32; 2]], p: [f32; 2]) -> f32 {
    let mut best = f32::MAX;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[j], polygon[i]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (ex, ey) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
        best = best.min((ex * ex + ey * ey).sqrt());
        j = i;
    }
    best
}

/// Renders an icon of `size` x `size` pixels as straight (not premultiplied)
/// RGBA, rows top to bottom.
#[expect(
    clippy::cast_precision_loss,
    reason = "icon sizes are at most 256 pixels"
)]
pub fn render(icon: TrayIcon, size: u32) -> Vec<u8> {
    let n = size as f32;
    let shape: Vec<[f32; 2]> = LOCUS.iter().map(|&p| place(p, n)).collect();
    let triangle: Vec<[f32; 2]> = BT2020.iter().map(|&p| place(p, n)).collect();
    let centre = place([0.30, 0.36], n);
    // SDR: an outline; HDR: filled, with the triangle drawn in white and a
    // white glow round the white point.
    let outline = (n * 0.09).max(1.5);
    let line = (n * 0.014).max(1.0);
    let glow = n * 0.42;
    let show_triangle = icon == TrayIcon::Hdr && size >= TRIANGLE_FROM;
    let samples = SUPERSAMPLE * SUPERSAMPLE;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut covered = 0_u32;
            let mut white = 0_u32;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let p = [
                        x as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32,
                        y as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32,
                    ];
                    if !inside(&shape, p) {
                        continue;
                    }
                    if icon == TrayIcon::Sdr && edge_distance(&shape, p) > outline {
                        continue;
                    }
                    covered += 1;
                    if show_triangle && edge_distance(&triangle, p) <= line / 2.0 {
                        white += 1;
                    }
                }
            }
            let rgb = match icon {
                TrayIcon::Hdr => {
                    let (dx, dy) = (x as f32 + 0.5 - centre[0], y as f32 + 0.5 - centre[1]);
                    let near = (1.0 - (dx * dx + dy * dy).sqrt() / glow).max(0.0);
                    let base = towards_white(spectrum_at(dx, dy), near * near * 0.75);
                    let triangle = if covered == 0 {
                        0.0
                    } else {
                        white as f32 / covered as f32 * 0.9
                    };
                    towards_white(base, triangle)
                }
                TrayIcon::Sdr => SDR_GREY,
            };
            pixels.extend_from_slice(&rgb);
            pixels.push(u8::try_from(covered * 255 / samples).unwrap_or(u8::MAX));
        }
    }
    pixels
}

/// Mixes `share` (0 to 1) of white into `base`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "values are bounded to 0..=255"
)]
fn towards_white(base: [u8; 3], share: f32) -> [u8; 3] {
    let share = share.clamp(0.0, 1.0);
    base.map(|c| (f32::from(c) + (255.0 - f32::from(c)) * share).round() as u8)
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

/// Colour of the conic gradient at an offset from the centre: pink at
/// [`COLOUR_START`] turns clockwise from the top, then round in palette order.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "values are bounded to 0..=255 and 0..5 by construction"
)]
fn spectrum_at(dx: f32, dy: f32) -> [u8; 3] {
    let turn = (dx.atan2(-dy) / std::f32::consts::TAU - COLOUR_START).rem_euclid(1.0);
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
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "test coordinates are small and positive"
    )]
    fn gamut_and_outline_shapes() {
        let size = 32;
        let filled = render(TrayIcon::Hdr, size);
        let outline = render(TrayIcon::Sdr, size);
        assert_eq!(filled.len(), (size * size * 4) as usize, "buffer size");
        // Inside the shape at its white point, away from the triangle's edges.
        let [cx, cy] = place([0.30, 0.36], size as f32);
        assert_eq!(
            alpha(&filled, size, cx as u32, cy as u32),
            255,
            "shape is filled"
        );
        assert_eq!(
            alpha(&outline, size, cx as u32, cy as u32),
            0,
            "outline is hollow"
        );
        let [tx, ty] = place([0.0743, 0.8338], size as f32);
        assert!(
            alpha(&outline, size, tx as u32, ty as u32 + 1) > 0,
            "outline is drawn at the top"
        );
        assert_eq!(
            alpha(&filled, size, size - 1, 0),
            0,
            "top right corner is clear"
        );
        assert_eq!(
            alpha(&filled, size, 0, size - 1),
            0,
            "bottom left corner is clear"
        );
    }

    #[test]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "test coordinates are small and positive"
    )]
    fn green_on_top_pink_on_the_right() {
        let size = 64;
        let icon = render(TrayIcon::Hdr, size);
        let pixel = |p: [f32; 2]| {
            let [x, y] = place(p, size as f32);
            let i = ((y as u32 * size + x as u32) * 4) as usize;
            [icon[i], icon[i + 1], icon[i + 2]]
        };
        let top = pixel([0.08, 0.78]);
        assert!(
            top[1] > top[0] && top[1] > top[2],
            "green at the top: {top:?}"
        );
        let right = pixel([0.62, 0.33]);
        assert!(
            right[0] > right[1] && right[0] > right[2],
            "pink on the right: {right:?}"
        );
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
    fn spectrum_has_no_yellow() {
        let start = COLOUR_START * std::f32::consts::TAU;
        assert_eq!(
            spectrum_at(start.sin(), -start.cos()),
            SPECTRUM[0],
            "pink at the start"
        );
        for i in 0..360_u16 {
            let a = f32::from(i).to_radians();
            let [r, g, b] = spectrum_at(a.sin(), -a.cos());
            let yellow = r > 180 && g > 180 && b < 90;
            assert!(!yellow, "no yellow at {i} degrees: {r},{g},{b}");
        }
    }
}
