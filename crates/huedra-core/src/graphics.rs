//! What each game program had for Windows Auto HDR and NVIDIA RTX HDR
//! before Huedra first changed it, kept in `graphics.toml` so uninstalling
//! Huedra can put it back.
//! Feature doc: `docs/features/04-game-library.md`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::files;

const MAX_BYTES: u64 = 1 << 20;

/// One program's values before Huedra's first change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Before {
    /// The program's full path.
    pub path: String,
    /// The Auto HDR value Windows had for it (`AutoHDREnable`), `None` when
    /// it had none. Meaningful once `auto_hdr_seen`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_hdr: Option<String>,
    /// Whether the Auto HDR value was recorded.
    #[serde(default)]
    pub auto_hdr_seen: bool,
    /// The RTX HDR value of its NVIDIA profile, `None` when it had none.
    /// Meaningful once `rtx_hdr_seen`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtx_hdr: Option<u32>,
    /// Whether the RTX HDR value was recorded.
    #[serde(default)]
    pub rtx_hdr_seen: bool,
}

/// `graphics.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default, rename = "program")]
    programs: Vec<Before>,
}

impl Record {
    /// Reads the record; a missing or damaged file gives an empty one.
    pub fn load(path: &Path) -> Self {
        files::read_limited(path, MAX_BYTES)
            .ok()
            .flatten()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the record atomically.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        files::write_atomic(path, text.as_bytes())
    }

    /// Every recorded program.
    pub fn programs(&self) -> &[Before] {
        &self.programs
    }

    fn entry(&mut self, path: &str) -> &mut Before {
        if let Some(i) = self
            .programs
            .iter()
            .position(|p| p.path.eq_ignore_ascii_case(path))
        {
            return &mut self.programs[i];
        }
        self.programs.push(Before {
            path: path.to_owned(),
            auto_hdr: None,
            auto_hdr_seen: false,
            rtx_hdr: None,
            rtx_hdr_seen: false,
        });
        let last = self.programs.len() - 1;
        &mut self.programs[last]
    }

    /// Records a program's Auto HDR value unless one was recorded before.
    /// Returns whether the record changed.
    pub fn note_auto_hdr(&mut self, path: &str, value: Option<String>) -> bool {
        let entry = self.entry(path);
        if entry.auto_hdr_seen {
            return false;
        }
        entry.auto_hdr = value;
        entry.auto_hdr_seen = true;
        true
    }

    /// Records a program's RTX HDR value unless one was recorded before.
    /// Returns whether the record changed.
    pub fn note_rtx_hdr(&mut self, path: &str, value: Option<u32>) -> bool {
        let entry = self.entry(path);
        if entry.rtx_hdr_seen {
            return false;
        }
        entry.rtx_hdr = value;
        entry.rtx_hdr_seen = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_first_value() {
        let mut record = Record::default();
        assert!(
            record.note_auto_hdr(r"C:\G\game.exe", Some("4147".into())),
            "first"
        );
        assert!(
            !record.note_auto_hdr(r"c:\g\GAME.exe", Some("48".into())),
            "not again, any case"
        );
        assert!(record.note_rtx_hdr(r"C:\G\game.exe", None), "rtx first");
        assert_eq!(record.programs().len(), 1, "one program");
        let before = &record.programs()[0];
        assert_eq!(before.auto_hdr.as_deref(), Some("4147"), "kept");
        assert!(before.rtx_hdr_seen && before.rtx_hdr.is_none(), "had none");
        let text = toml::to_string(&record).expect("serialize");
        assert_eq!(
            toml::from_str::<Record>(&text).expect("parse"),
            record,
            "round trip"
        );
    }
}
