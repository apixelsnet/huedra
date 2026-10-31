//! Small, safe file helpers: size-limited reads, atomic writes, and the
//! recovery record. Feature docs: `10-config.md`, `01-hdr-switch.md`.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::display::DisplayId;

/// Reads a text file of at most `max_bytes`. Returns `Ok(None)` when the
/// file does not exist.
///
/// # Errors
/// I/O errors, invalid UTF-8, or a file larger than `max_bytes`
/// ([`io::ErrorKind::FileTooLarge`]).
pub fn read_limited(path: &Path, max_bytes: u64) -> io::Result<Option<String>> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if meta.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("{} is larger than {max_bytes} bytes", path.display()),
        ));
    }
    fs::read_to_string(path).map(Some)
}

/// Writes `contents` to a temporary file next to `path`, flushes it to disk
/// and renames it over `path`, so a power cut leaves either the old or the
/// new file, never a partial one.
///
/// # Errors
/// I/O errors from creating, writing or renaming the file.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = Path::new(&tmp_name);
    {
        let mut file = fs::File::create(tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(tmp, path)
}

/// The recovery record `owned.toml`: displays whose HDR Huedra turned on.
/// Written before HDR is turned on and after it is turned off, so after a
/// crash Huedra knows which HDR is its own to undo.
#[derive(Debug, Default, Serialize, Deserialize)]
struct OwnedRecord {
    displays: Vec<String>,
}

const OWNED_MAX_BYTES: u64 = 64 * 1024;

/// Reads the recovery record. A missing or unreadable record means nothing
/// is owned, which is the safe reading: Huedra then leaves HDR alone.
pub fn load_owned(path: &Path) -> Vec<DisplayId> {
    read_limited(path, OWNED_MAX_BYTES)
        .ok()
        .flatten()
        .and_then(|text| toml::from_str::<OwnedRecord>(&text).ok())
        .map(|r| r.displays.into_iter().map(DisplayId::new).collect())
        .unwrap_or_default()
}

/// Writes the recovery record.
///
/// # Errors
/// I/O errors from [`write_atomic`].
pub fn save_owned(path: &Path, owned: &[DisplayId]) -> io::Result<()> {
    let record = OwnedRecord {
        displays: owned.iter().map(|d| d.as_str().to_owned()).collect(),
    };
    let text = toml::to_string(&record).map_err(io::Error::other)?;
    write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("huedra-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn atomic_write_and_limited_read() {
        let dir = temp_dir("files");
        let path = dir.join("a.toml");
        assert_eq!(read_limited(&path, 10).ok(), Some(None), "missing is None");
        write_atomic(&path, b"hello").expect("write");
        write_atomic(&path, b"world").expect("overwrite");
        assert_eq!(
            read_limited(&path, 10).ok(),
            Some(Some("world".into())),
            "replaced"
        );
        assert!(!dir.join("a.toml.tmp").exists(), "no temp file left");
        assert_eq!(
            read_limited(&path, 3).map_err(|e| e.kind()),
            Err(io::ErrorKind::FileTooLarge),
            "limit"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_round_trip() {
        let dir = temp_dir("owned");
        let path = dir.join("owned.toml");
        assert!(load_owned(&path).is_empty(), "missing record owns nothing");
        let ids = vec![DisplayId::new(r"\\?\DISPLAY#GSM5B7F#1")];
        save_owned(&path, &ids).expect("save");
        assert_eq!(load_owned(&path), ids, "round trip");
        fs::write(&path, "not toml [").expect("corrupt");
        assert!(load_owned(&path).is_empty(), "corrupt record owns nothing");
        let _ = fs::remove_dir_all(&dir);
    }
}
