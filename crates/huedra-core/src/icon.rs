//! Draws Huedra's icon: the outline of the colors the eye can see (the CIE
//! 1931 chromaticity diagram), filled with the BT.2020 palette while HDR is
//! on, with the BT.2020 triangle inside at larger sizes; a gray outline of
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
const SDR_GRAY: [u8; 3] = [0xA9, 0xA4, 0xB6];
/// Samples per pixel along each axis, for smooth edges.
const SUPERSAMPLE: u32 = 4;
/// Where the colors start, in turns clockwise from the top: green sits at
/// the top, as on the diagram, and pink on the right.
const COLOR_START: f32 = 0.2;

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

/// Maps diagram coordinates into an icon of `n` pixels, centered with a
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
    let center = place([0.30, 0.36], n);
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
                    let (dx, dy) = (x as f32 + 0.5 - center[0], y as f32 + 0.5 - center[1]);
                    let near = (1.0 - (dx * dx + dy * dy).sqrt() / glow).max(0.0);
                    let base = towards_white(spectrum_at(dx, dy), near * near * 0.75);
                    let triangle = if covered == 0 {
                        0.0
                    } else {
                        white as f32 / covered as f32 * 0.9
                    };
                    towards_white(base, triangle)
                }
                TrayIcon::Sdr => SDR_GRAY,
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

/// How many glowing waves the Status card's banner holds.
const WAVES: u32 = 3;
/// The HDR banner behind the waves: a deep navy, in scRGB (linear, 1.0 = 80
/// nits), well under 1 nit.
const WAVE_BASE: [f32; 3] = [0.003, 0.005, 0.016];
/// How much of the full opacity the SDR waves take at their brightest.
const SDR_WAVE_OPACITY: f32 = 0.9;
/// The lower share of the banner over which its waves and background fade
/// out, so the banner melts into the card below it.
const WAVE_FADE: f32 = 0.45;

/// Glowing waves of the spectrum for a display in HDR mode, as a banner
/// that is opaque at the top and fades out over its lower part: RGBA as
/// the bits of 16-bit floats (for an FP16 surface), premultiplied, in
/// scRGB (linear, 1.0 = 80 nits). Each wave is a crisp line of light with a soft
/// glow and a faint sheet below it, brightening and fading along its length,
/// on a deep navy. Colors run along the waves round the spectrum, taken as
/// BT.2020 colors (so reaching past sRGB); the lines reach `nits` in their
/// strongest channel, which should be near the display's peak: bright thin
/// light on a dark banner is what makes it look HDR. The banner wraps
/// sideways, so copies side by side join without a seam.
pub fn hdr_waves(width: u32, height: u32, seed: u32, nits: f32) -> Vec<u16> {
    let peak = nits / 80.0;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    let columns = width.max(1);
    for (index, cell) in wave_light(width, height, seed, wide_color)
        .into_iter()
        .enumerate()
    {
        let alpha = fade(index, columns, height);
        // Soft limit at the peak, the same for every channel so colors keep
        // their hue where waves cross.
        let strongest = cell.iter().copied().fold(0.0_f32, f32::max);
        let scale = if strongest > 0.0 {
            peak * (1.0 - (-2.5 * strongest).exp()) / strongest
        } else {
            0.0
        };
        for (base, c) in WAVE_BASE.iter().zip(cell) {
            pixels.push(f16_bits((base + c * scale) * alpha));
        }
        pixels.push(f16_bits(alpha));
    }
    pixels
}

/// The same waves as [`hdr_waves`] for an ordinary (SDR) window, as straight
/// RGBA over a transparent banner: in `gray` (sRGB) when given, else in the
/// spectrum's colors. Opacity and color are dithered so the soft glow shows
/// no bands.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "values are bounded to 0..=255; image sizes are small"
)]
pub fn sdr_waves(width: u32, height: u32, seed: u32, gray: Option<[u8; 3]>) -> Vec<u8> {
    let tint = |srgb: [f32; 3]| match gray {
        Some(_) => [1.0; 3],
        None => {
            let linear = srgb.map(|c| srgb_to_linear(c / 255.0));
            let strongest = linear.iter().copied().fold(f32::MIN_POSITIVE, f32::max);
            linear.map(|c| c / strongest)
        }
    };
    let columns = width.max(1);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for (index, cell) in wave_light(width, height, seed, tint)
        .into_iter()
        .enumerate()
    {
        let (x, y) = (index as u32 % columns, index as u32 / columns);
        let strongest = cell.iter().copied().fold(0.0_f32, f32::max);
        let level = (1.0 - (-2.5 * strongest).exp()) * fade(index, columns, height);
        let alpha = (SDR_WAVE_OPACITY * 255.0 * level + dither(x, y, 3))
            .round()
            .clamp(0.0, 255.0);
        if alpha < 1.0 {
            pixels.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let color = match gray {
            Some(gray) => gray.map(f32::from),
            None => cell.map(|c| linear_to_srgb(c / strongest.max(f32::MIN_POSITIVE)) * 255.0),
        };
        for (channel, c) in color.into_iter().enumerate() {
            pixels.push((c + dither(x, y, channel as u32)).round().clamp(0.0, 255.0) as u8);
        }
        pixels.push(alpha as u8);
    }
    pixels
}

/// How opaque the banner is at pixel `index` (row by row, `columns` wide,
/// `height` tall): fully at the top, fading smoothly to nothing over the
/// lower `WAVE_FADE` of it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "image sizes are small"
)]
fn fade(index: usize, columns: u32, height: u32) -> f32 {
    let row = (index as u32 / columns) as f32 + 0.5;
    let h = height.max(1) as f32;
    let into = ((row - h * (1.0 - WAVE_FADE)) / (h * WAVE_FADE)).clamp(0.0, 1.0);
    let left = 1.0 - into;
    left * left * (3.0 - 2.0 * left)
}

/// The light of the waves for each pixel, row by row, in linear terms: each
/// wave's color from `tint` (given the spectrum's sRGB color at that point)
/// times how much of its line, glow and sheet reaches the pixel.
#[expect(
    clippy::cast_precision_loss,
    reason = "image sizes are small; the wave count is small"
)]
fn wave_light(
    width: u32,
    height: u32,
    seed: u32,
    tint: impl Fn([f32; 3]) -> [f32; 3],
) -> Vec<[f32; 3]> {
    use std::f32::consts::TAU;
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    // Every term has a whole number of cycles across the banner, so it wraps.
    let waves: Vec<WaveShape> = (0..WAVES)
        .map(|k| {
            let salt = seed * 17 + k;
            let r = |n: u32| hash01(salt, n, 11);
            WaveShape {
                center: h * (0.38 + 0.12 * k as f32 + 0.08 * (r(0) - 0.5)),
                terms: [
                    (h * (0.14 + 0.08 * r(1)), 1.0, TAU * r(2)),
                    (h * (0.06 + 0.05 * r(3)), 2.0, TAU * r(4)),
                ],
                light: (2.0, TAU * r(5)),
                hue: k as f32 / WAVES as f32 + 0.15 * r(6),
            }
        })
        .collect();
    // Sizes from the banner's height: a line about 1.2 pixels at 128 tall.
    let line = (h / 110.0).max(0.6);
    let glow = h * 0.06;
    let sheet = h * 0.35;
    let mut light = vec![[0.0_f32; 3]; (width * height) as usize];
    for x in 0..width {
        let t = (x as f32 + 0.5) / w;
        for wave in &waves {
            let (mut y, mut slope) = (wave.center, 0.0_f32);
            for (amplitude, cycles, phase) in wave.terms {
                let angle = TAU * cycles * t + phase;
                y += amplitude * angle.sin();
                slope += amplitude * TAU * cycles / w * angle.cos();
            }
            let across = 1.0 / (1.0 + slope * slope).sqrt();
            let (cycles, phase) = wave.light;
            let strength = 0.35 + 0.65 * (0.5 + 0.5 * (TAU * cycles * t + phase).sin());
            let color = tint(cyclic(wave.hue + t));
            let column = light
                .iter_mut()
                .skip(x as usize)
                .step_by(width.max(1) as usize);
            for (row, cell) in column.enumerate() {
                let d = (row as f32 + 0.5 - y) * across;
                let mut amount = (-(d / line).powi(2)).exp() + 0.22 * (-(d / glow).powi(2)).exp();
                if d > 0.0 {
                    amount += 0.04 * (-d / sheet).exp();
                }
                let amount = amount * strength;
                for (c, k) in cell.iter_mut().zip(color) {
                    *c += k * amount;
                }
            }
        }
    }
    light
}

/// One of the banner's waves: its middle height, its curve as sine terms
/// (amplitude, whole cycles across the banner, phase), how its light rises
/// and falls along it (whole cycles, phase), and where its colors start.
struct WaveShape {
    center: f32,
    terms: [(f32, f32, f32); 2],
    light: (f32, f32),
    hue: f32,
}

/// An sRGB color (0 to 255) taken as a BT.2020 color and given in scRGB's
/// BT.709 primaries, scaled so its strongest channel is 1; channels below 0
/// are colors outside sRGB.
fn wide_color(color: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = color.map(|c| srgb_to_linear(c / 255.0));
    let wide = [
        1.6605 * r - 0.5876 * g - 0.0728 * b,
        -0.1246 * r + 1.1329 * g - 0.0083 * b,
        -0.0182 * r - 0.1006 * g + 1.1187 * b,
    ];
    let strongest = wide.iter().copied().fold(f32::MIN_POSITIVE, f32::max);
    wide.map(|c| c / strongest)
}

/// sRGB's transfer curve undone: a 0 to 1 value to linear light.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB's transfer curve: linear light, 0 to 1, to a 0 to 1 value.
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// The bits of `value` as a 16-bit float (IEEE 754 half precision), rounded
/// to nearest; too large becomes infinity, too small zero.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "bit fields are masked to their widths"
)]
pub fn f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x007F_FFFF;
    if exponent == 0xFF {
        return sign | 0x7C00 | if mantissa == 0 { 0 } else { 0x0200 };
    }
    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1F {
        return sign | 0x7C00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        // Too small for a normal half: a subnormal one.
        let full = mantissa | 0x0080_0000;
        let shift = (14 - half_exponent) as u32;
        let rounded = (full >> shift) + ((full >> (shift - 1)) & 1);
        return sign | rounded as u16;
    }
    let half = u32::from(sign) | ((half_exponent as u32) << 10) | (mantissa >> 13);
    // Round to nearest; a carry into the exponent is still right.
    (half + ((mantissa >> 12) & 1)) as u16
}

/// The spectrum as a loop, 0 to 1: pink, violet, ultramarine, cyan, green
/// and back to pink, as around the logo.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "t is 0..1, so the index is 0..5"
)]
fn cyclic(t: f32) -> [f32; 3] {
    let scaled = t.rem_euclid(1.0) * SPECTRUM.len() as f32;
    let index = (scaled.floor() as usize) % SPECTRUM.len();
    let next = (index + 1) % SPECTRUM.len();
    let f = scaled.fract();
    let (a, b) = (SPECTRUM[index], SPECTRUM[next]);
    [0, 1, 2].map(|k| f32::from(a[k]) + (f32::from(b[k]) - f32::from(a[k])) * f)
}

/// Noise for dithering, -1 to 1 with most values near 0 (the sum of two
/// even draws), fixed for each pixel and channel.
fn dither(x: u32, y: u32, channel: u32) -> f32 {
    hash01(x, y, channel * 2) + hash01(x, y, channel * 2 + 1) - 1.0
}

/// A number from 0 to 1 that looks random but is fixed for its inputs.
#[expect(
    clippy::cast_precision_loss,
    reason = "a 24-bit value converts exactly"
)]
fn hash01(x: u32, y: u32, salt: u32) -> f32 {
    let mut h =
        x.wrapping_mul(0x8DA6_B343) ^ y.wrapping_mul(0xD816_3841) ^ salt.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// Color of the conic gradient at an offset from the center: pink at
/// [`COLOR_START`] turns clockwise from the top, then round in palette order.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "values are bounded to 0..=255 and 0..5 by construction"
)]
fn spectrum_at(dx: f32, dy: f32) -> [u8; 3] {
    let turn = (dx.atan2(-dy) / std::f32::consts::TAU - COLOR_START).rem_euclid(1.0);
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
    fn half_floats_match_known_values() {
        assert_eq!(f16_bits(0.0), 0x0000, "zero");
        assert_eq!(f16_bits(1.0), 0x3C00, "one");
        assert_eq!(f16_bits(0.5), 0x3800, "half");
        assert_eq!(f16_bits(-2.0), 0xC000, "minus two");
        assert_eq!(f16_bits(65504.0), 0x7BFF, "largest");
        assert_eq!(f16_bits(1.0e6), 0x7C00, "too large is infinity");
        assert_eq!(f16_bits(1.0e-9), 0x0000, "too small is zero");
        assert_eq!(f16_bits(6.103_515_6e-5), 0x0400, "smallest normal");
    }

    #[test]
    fn hdr_waves_are_wide_bright_and_fade_out() {
        let (w, h) = (320, 128);
        let waves = hdr_waves(w, h, 0, 400.0);
        assert_eq!(waves.len(), (w * h * 4) as usize, "size");
        let alpha = |row: u32| waves[((row * w) * 4 + 3) as usize];
        assert_eq!(alpha(0), 0x3C00, "opaque at the top");
        assert!(alpha(h - 1) < 0x2000, "nearly clear at the foot");
        assert!(
            (1..h).all(|row| alpha(row) <= alpha(row - 1)),
            "fades steadily"
        );
        // Some channel below zero, a color outside sRGB: negative halves
        // have the sign bit set.
        let negative = |c: &u16| c & 0x8000 != 0 && c & 0x7FFF != 0;
        assert!(
            waves.chunks(4).any(|p| p[..3].iter().any(negative)),
            "wide colors"
        );
        // The lines come close to 400 nits (5.0 in scRGB is 0x4500) and
        // never pass it; 4.0 is 0x4400.
        let brightest = waves
            .chunks(4)
            .flat_map(|p| p[..3].iter().copied())
            .filter(|c| c & 0x8000 == 0)
            .max()
            .unwrap_or(0);
        assert!(
            (0x4400..=0x4500).contains(&brightest),
            "peak {brightest:#x}"
        );
        // Mostly dark (under 40 nits, 0.5 is 0x3800): the card stays calm
        // around the lines.
        let dark = waves
            .chunks(4)
            .filter(|p| p[..3].iter().all(|&c| c & 0x8000 != 0 || c < 0x3800))
            .count();
        assert!(dark * 2 > (w * h) as usize, "{dark} dark pixels");
    }

    #[test]
    fn sdr_waves_glow_on_a_clear_banner() {
        let (w, h) = (320, 96);
        let gray = [200, 200, 210];
        let waves = sdr_waves(w, h, 0, Some(gray));
        assert_eq!(waves.len(), (w * h * 4) as usize, "size");
        let alphas: Vec<u8> = waves.chunks(4).map(|p| p[3]).collect();
        assert!(alphas.contains(&0), "clear between the waves");
        assert!(
            alphas.iter().any(|&a| a > 200),
            "nearly opaque on the lines"
        );
        // Gray waves stay gray, give or take the dither.
        assert!(
            waves
                .chunks(4)
                .filter(|p| p[3] > 0)
                .all(|p| p[..3].iter().zip(gray).all(|(&c, g)| c.abs_diff(g) <= 1)),
            "gray"
        );
        // In color they take several hues.
        let colored = sdr_waves(w, h, 0, None);
        let reddest = colored.chunks(4).filter(|p| p[3] > 128).map(|p| p[0]).max();
        let greenest = colored.chunks(4).filter(|p| p[3] > 128).map(|p| p[1]).max();
        assert!(
            reddest > Some(200) && greenest > Some(200),
            "{reddest:?} {greenest:?}"
        );
    }

    #[test]
    fn spectrum_has_no_yellow() {
        let start = COLOR_START * std::f32::consts::TAU;
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
