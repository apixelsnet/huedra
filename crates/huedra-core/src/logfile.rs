//! The log file: plain lines, at most two files of a fixed size.
//! Feature doc: `docs/features/10-config.md`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::PathBuf;

/// Default size of each log file.
pub const MAX_BYTES: u64 = 1 << 20;

/// Appends lines to `huedra.log`, moving it to `huedra.log.1` when full.
#[derive(Debug, Clone)]
pub struct LogFile {
    path: PathBuf,
    max_bytes: u64,
}

impl LogFile {
    /// A log at `path` rotating at `max_bytes`.
    pub fn new(path: PathBuf, max_bytes: u64) -> Self {
        Self { path, max_bytes }
    }

    /// Appends one line. Line breaks inside `line` are replaced so each entry
    /// stays on one line.
    ///
    /// # Errors
    /// I/O errors from rotating or writing.
    pub fn append(&self, line: &str) -> io::Result<()> {
        if fs::metadata(&self.path).is_ok_and(|m| m.len() >= self.max_bytes) {
            let mut old = self.path.clone().into_os_string();
            old.push(".1");
            fs::rename(&self.path, old)?;
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let clean: String = line
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        writeln!(file, "{clean}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates() {
        let dir = std::env::temp_dir().join(format!("huedra-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log = LogFile::new(dir.join("huedra.log"), 10);
        log.append("first line\nsplit").expect("write");
        log.append("second").expect("rotate");
        let current = fs::read_to_string(dir.join("huedra.log")).expect("current");
        let old = fs::read_to_string(dir.join("huedra.log.1")).expect("old");
        assert_eq!(current, "second\n", "new file");
        assert_eq!(old, "first line split\n", "one line per entry");
        let _ = fs::remove_dir_all(&dir);
    }
}
