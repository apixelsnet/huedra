//! Windows resources, added when linking for Windows with the MSVC toolchain:
//!
//! - `huedra.manifest`: run as invoker, per-monitor DPI awareness, UTF-8
//!   code page, and no console window when started from Explorer.
//! - Version information and the application icon (the HDR spectrum dot,
//!   drawn by the same code as the tray icon), compiled with `llvm-rc`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use huedra_core::icon::{self, TrayIcon};

/// Icon sizes Windows uses at 100 % to 400 % scaling.
const ICON_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=huedra.manifest");
    println!("cargo:rerun-if-env-changed=RC");
    let msvc = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|v| v == "msvc");
    let (Some(dir), Some(out)) = (env::var_os("CARGO_MANIFEST_DIR"), env::var_os("OUT_DIR")) else {
        return;
    };
    if !msvc {
        return;
    }
    let manifest = PathBuf::from(dir).join("huedra.manifest");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
    match resources(Path::new(&out)) {
        Ok(res) => println!("cargo:rustc-link-arg-bins={}", res.display()),
        Err(problem) => println!("cargo:warning=no version information or icon: {problem}"),
    }
}

/// Writes the icon and the resource script, compiles them, and returns the
/// `.res` file for the linker.
fn resources(out: &Path) -> Result<PathBuf, String> {
    fs::write(out.join("huedra.ico"), ico()?).map_err(|e| e.to_string())?;
    fs::write(out.join("huedra.rc"), script()?).map_err(|e| e.to_string())?;
    let res = out.join("huedra.res");
    let compiler = env::var("RC").unwrap_or_else(|_| "llvm-rc".to_owned());
    let status = Command::new(&compiler)
        .current_dir(out)
        .args(["/nologo", "/fo"])
        .arg(&res)
        .arg("huedra.rc")
        .status()
        .map_err(|e| format!("could not run {compiler}: {e}"))?;
    if status.success() {
        Ok(res)
    } else {
        Err(format!("{compiler} failed with {status}"))
    }
}

fn script() -> Result<String, String> {
    let version = env::var("CARGO_PKG_VERSION").map_err(|e| e.to_string())?;
    let numbers: Vec<&str> = version.split(['.', '-', '+']).take(3).collect();
    let [major, minor, patch] = numbers.as_slice() else {
        return Err(format!("unexpected version {version}"));
    };
    Ok(format!(
        r#"1 ICON "huedra.ico"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3F
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Huedra"
      VALUE "FileDescription", "Huedra: automatic HDR switching for games and video"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "huedra"
      VALUE "LegalCopyright", "Huedra contributors"
      VALUE "OriginalFilename", "huedra.exe"
      VALUE "ProductName", "Huedra"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    ))
}

/// An `.ico` file with 32-bit images at every size in [`ICON_SIZES`].
fn ico() -> Result<Vec<u8>, String> {
    let images: Vec<(u32, Vec<u8>)> = ICON_SIZES
        .iter()
        .map(|&size| Ok((size, bmp_entry(size)?)))
        .collect::<Result<_, String>>()?;
    let count = u16::try_from(images.len()).map_err(|e| e.to_string())?;
    let mut file = Vec::new();
    file.extend_from_slice(&0_u16.to_le_bytes()); // reserved
    file.extend_from_slice(&1_u16.to_le_bytes()); // type: icon
    file.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for (size, data) in &images {
        let side = u8::try_from(*size).unwrap_or(0); // 0 means 256
        let len = u32::try_from(data.len()).map_err(|e| e.to_string())?;
        file.extend_from_slice(&[side, side, 0, 0]);
        file.extend_from_slice(&1_u16.to_le_bytes()); // planes
        file.extend_from_slice(&32_u16.to_le_bytes()); // bits per pixel
        file.extend_from_slice(&len.to_le_bytes());
        file.extend_from_slice(&offset.to_le_bytes());
        offset += len;
    }
    for (_, data) in images {
        file.extend_from_slice(&data);
    }
    Ok(file)
}

/// One icon image: a BITMAPINFOHEADER, BGRA rows bottom-up, and an empty
/// AND mask (the alpha channel decides transparency).
fn bmp_entry(size: u32) -> Result<Vec<u8>, String> {
    let rgba = icon::render(TrayIcon::Hdr, size);
    let side = i32::try_from(size).map_err(|e| e.to_string())?;
    let mask_row = size.div_ceil(32) * 4;
    let image_bytes = size * size * 4 + mask_row * size;
    let mut data = Vec::with_capacity(40 + image_bytes as usize);
    data.extend_from_slice(&40_u32.to_le_bytes());
    data.extend_from_slice(&side.to_le_bytes());
    data.extend_from_slice(&(side * 2).to_le_bytes()); // colour and mask
    data.extend_from_slice(&1_u16.to_le_bytes());
    data.extend_from_slice(&32_u16.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes()); // BI_RGB
    data.extend_from_slice(&image_bytes.to_le_bytes());
    data.extend_from_slice(&[0; 16]); // resolution and palette fields
    let row = size as usize * 4;
    for y in (0..size as usize).rev() {
        for px in rgba[y * row..(y + 1) * row].chunks(4) {
            data.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    data.resize(data.len() + (mask_row * size) as usize, 0);
    Ok(data)
}
