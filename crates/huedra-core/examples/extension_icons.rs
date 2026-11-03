//! Writes the browser extension's icons (`extras/browser/icons/`): the HDR
//! spectrum dot, drawn by the same code as the tray and program icons.
//!
//! `cargo run -p huedra-core --example extension_icons -- <directory>`
//! (default `extras/browser/icons`). `build/ci.sh` checks that the committed
//! icons match this output. Feature doc: `docs/features/05-video.md`.

use std::path::PathBuf;

use huedra_core::icon::{self, TrayIcon};

/// Icon sizes and the size of the dot drawn in each. Store guidelines ask
/// for 16 pixels of clear space around the art of the 128-pixel icon.
const ICONS: [(u32, u32); 4] = [(16, 16), (32, 32), (48, 48), (128, 96)];

fn main() -> std::io::Result<()> {
    let dir = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("extras/browser/icons"), PathBuf::from);
    std::fs::create_dir_all(&dir)?;
    for (size, art) in ICONS {
        let pixels = centered(&icon::render(TrayIcon::Hdr, art), art, size);
        std::fs::write(dir.join(format!("icon-{size}.png")), png(size, &pixels)?)?;
    }
    Ok(())
}

/// Places a square RGBA image of `art` pixels in the middle of a clear
/// square of `size` pixels.
fn centered(art_pixels: &[u8], art: u32, size: u32) -> Vec<u8> {
    let (art, size) = (art as usize, size as usize);
    let offset = (size - art) / 2;
    let mut pixels = vec![0_u8; size * size * 4];
    for (y, row) in art_pixels.chunks_exact(art * 4).enumerate() {
        let start = ((y + offset) * size + offset) * 4;
        pixels[start..start + art * 4].copy_from_slice(row);
    }
    pixels
}

/// Encodes straight RGBA as an uncompressed PNG: the icons are small, and
/// this keeps the example free of dependencies.
fn png(size: u32, rgba: &[u8]) -> std::io::Result<Vec<u8>> {
    let too_big = || std::io::Error::other("image too large");
    let mut raw = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks_exact(size as usize * 4) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    // zlib stream of stored deflate blocks.
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(usize::from(u16::MAX)).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        let len = u16::try_from(block.len()).map_err(|_| too_big())?;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, no interlace

    let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
    for (kind, data) in [(b"IHDR", &header), (b"IDAT", &zlib), (b"IEND", &Vec::new())] {
        let len = u32::try_from(data.len()).map_err(|_| too_big())?;
        out.extend_from_slice(&len.to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }
    Ok(out)
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
