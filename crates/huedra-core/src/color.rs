//! Color profiles (ICC) that a game asks for while it runs: which profile
//! each display should use, what Windows used before, and putting that back.
//!
//! Windows keeps two lists of profiles for each display, one for SDR and one
//! for HDR (advanced color), each with a default. A game may name a profile
//! for either; while it runs, that profile is the default on the display its
//! window is on, and when it closes the earlier default returns. The record
//! `color.toml` holds every change until it is undone, so after a crash the
//! next start puts the earlier profiles back.
//!
//! No Windows code: the caller passes a [`ColorSystem`] that reads and
//! changes the defaults.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::display::DisplayId;
use crate::engine::TriggerId;
use crate::files;

/// Longest profile file name accepted from the settings file.
pub const MAX_NAME: usize = 200;
/// Most profiles listed from the color folder.
pub const MAX_LISTED: usize = 200;
/// How much of a profile file is read to tell its kind: the header and the
/// tag table.
pub const HEADER_BYTES: usize = 16 * 1024;
const RECORD_MAX_BYTES: u64 = 256 * 1024;

/// Which of a display's two profile lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorMode {
    /// The profiles used while HDR is off.
    Sdr,
    /// The profiles used while HDR is on (advanced color).
    Hdr,
}

impl ColorMode {
    /// "SDR" or "HDR".
    pub fn label(self) -> &'static str {
        match self {
            Self::Sdr => "SDR",
            Self::Hdr => "HDR",
        }
    }
}

/// The profiles a game asks for; `None` leaves Windows' choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameProfiles {
    /// The profile for SDR.
    pub sdr: Option<String>,
    /// The profile for HDR.
    pub hdr: Option<String>,
}

impl GameProfiles {
    /// Whether the game asks for any profile.
    pub fn is_empty(&self) -> bool {
        self.sdr.is_none() && self.hdr.is_none()
    }

    fn get(&self, mode: ColorMode) -> Option<&String> {
        match mode {
            ColorMode::Sdr => self.sdr.as_ref(),
            ColorMode::Hdr => self.hdr.as_ref(),
        }
    }
}

/// Whether a name from the settings file can be a profile in the Windows
/// color folder: a plain `.icc` or `.icm` file name, no folder.
pub fn is_profile_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.trim().is_empty()
        && name.len() <= MAX_NAME
        && !name.contains(['\\', '/', ':', '\0'])
        && (lower.ends_with(".icc") || lower.ends_with(".icm"))
}

/// The installed profiles a game can choose, by file name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// Display profiles, for SDR.
    pub sdr: Vec<String>,
    /// Display profiles that Windows applies in HDR (with an `MHC2` tag,
    /// such as those made by the Windows HDR Calibration app).
    pub hdr: Vec<String>,
}

impl Installed {
    /// Sorts the lists by name, ignoring case, and caps their length.
    pub fn sorted(mut self) -> Self {
        for list in [&mut self.sdr, &mut self.hdr] {
            list.sort_by_key(|n| n.to_lowercase());
            list.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
            list.truncate(MAX_LISTED);
        }
        self
    }
}

/// What an ICC profile is for, read from its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileKind {
    /// A display (monitor) profile, as opposed to a printer or scanner one.
    pub display: bool,
    /// Carries the `MHC2` tag, so Windows applies it in HDR.
    pub hdr: bool,
}

/// Reads the kind of an ICC profile from its first bytes (at least the
/// 128-byte header; the tag table follows). `None` for anything that is not
/// an ICC profile.
pub fn inspect(bytes: &[u8]) -> Option<ProfileKind> {
    if bytes.get(36..40)? != b"acsp" {
        return None;
    }
    let display = bytes.get(12..16)? == b"mntr";
    let count: [u8; 4] = bytes.get(128..132)?.try_into().ok()?;
    let count = u32::from_be_bytes(count) as usize;
    let hdr = (0..count.min(4096))
        .map_while(|i| bytes.get(132 + i * 12..136 + i * 12))
        .any(|sig| sig == b"MHC2");
    Some(ProfileKind { display, hdr })
}

/// A change of a display's default profile could not be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ColorError {
    /// The display is not connected now; the change is tried again later.
    #[error("the display is not connected")]
    Gone,
    /// Windows refused.
    #[error("{0}")]
    Failed(String),
}

/// Reads and changes the default profiles of the displays.
pub trait ColorSystem {
    /// The default profile of one list, `None` when the list is empty.
    ///
    /// # Errors
    /// [`ColorError`] when it cannot be read.
    fn default_profile(
        &mut self,
        display: &DisplayId,
        mode: ColorMode,
    ) -> Result<Option<String>, ColorError>;

    /// Makes `profile` the default of one list, adding it to the list
    /// first when it is not in it. Returns whether it was added.
    ///
    /// # Errors
    /// [`ColorError`] when Windows refuses.
    fn use_profile(
        &mut self,
        display: &DisplayId,
        mode: ColorMode,
        profile: &str,
    ) -> Result<bool, ColorError>;

    /// Takes `profile` off one list.
    ///
    /// # Errors
    /// [`ColorError`] when Windows refuses.
    fn remove_profile(
        &mut self,
        display: &DisplayId,
        mode: ColorMode,
        profile: &str,
    ) -> Result<(), ColorError>;

    /// Saves the record of changes; called before and after every change,
    /// so a crash in between still leaves a record.
    fn save(&mut self, record: &[Change]);
}

/// One default that Huedra changed and has not put back yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// The display.
    pub display: String,
    /// Which list.
    pub mode: ColorMode,
    /// The default before Huedra changed it; absent when the list had none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    /// The profile Huedra made the default.
    pub applied: String,
    /// Whether Huedra added `applied` to the list, so it is taken off again.
    #[serde(default)]
    pub added: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    #[serde(default, rename = "change")]
    changes: Vec<Change>,
}

/// Reads the record of changes. A missing or unreadable record holds none.
pub fn load_record(path: &Path) -> Vec<Change> {
    files::read_limited(path, RECORD_MAX_BYTES)
        .ok()
        .flatten()
        .and_then(|text| toml::from_str::<Record>(&text).ok())
        .map(|r| r.changes)
        .unwrap_or_default()
}

/// Writes the record of changes.
///
/// # Errors
/// I/O errors from [`files::write_atomic`].
pub fn save_record(path: &Path, changes: &[Change]) -> io::Result<()> {
    let record = Record {
        changes: changes.to_vec(),
    };
    let text = toml::to_string(&record).map_err(io::Error::other)?;
    files::write_atomic(path, text.as_bytes())
}

/// A game that asks for profiles, and the displays it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameColor {
    /// The game.
    pub id: TriggerId,
    /// Its name, for the log.
    pub label: String,
    /// The displays its windows are on.
    pub displays: BTreeSet<DisplayId>,
    /// What it asks for.
    pub profiles: GameProfiles,
}

type Key = (DisplayId, ColorMode);

/// The profiles running games ask for and the changes made for them.
#[derive(Debug, Default)]
pub struct ColorState {
    games: Vec<GameColor>,
    changed: BTreeMap<Key, Change>,
}

impl ColorState {
    /// Starts from the record left by the last run; the first
    /// [`Self::sync`] puts those defaults back unless a game wants them.
    pub fn from_record(record: Vec<Change>) -> Self {
        let changed = record
            .into_iter()
            .map(|c| ((DisplayId::new(c.display.as_str()), c.mode), c))
            .collect();
        Self {
            games: Vec::new(),
            changed,
        }
    }

    /// Replaces the running games. When two games on one display ask for a
    /// profile in the same list, the later one in `games` wins.
    pub fn set_games(&mut self, games: Vec<GameColor>) {
        self.games = games;
    }

    /// The running games that ask for profiles.
    pub fn games(&self) -> &[GameColor] {
        &self.games
    }

    /// Whether a game asks for profiles or a change waits to be undone.
    pub fn is_active(&self) -> bool {
        !self.games.is_empty() || !self.changed.is_empty()
    }

    fn record(&self) -> Vec<Change> {
        self.changed.values().cloned().collect()
    }

    /// Which profile each display list should use, and the game asking.
    fn wanted(&self) -> BTreeMap<Key, (String, String)> {
        let mut wanted = BTreeMap::new();
        for game in &self.games {
            for display in &game.displays {
                for mode in [ColorMode::Sdr, ColorMode::Hdr] {
                    if let Some(profile) = game.profiles.get(mode) {
                        wanted.insert(
                            (display.clone(), mode),
                            (profile.clone(), game.label.clone()),
                        );
                    }
                }
            }
        }
        wanted
    }

    /// Brings the displays in line with the running games: puts back the
    /// defaults no game wants any more, then sets the ones they want.
    /// Returns lines for the log.
    pub fn sync(&mut self, system: &mut impl ColorSystem) -> Vec<String> {
        let mut log = Vec::new();
        let wanted = self.wanted();
        let done: Vec<(Key, Change)> = self
            .changed
            .iter()
            .filter(|(k, _)| !wanted.contains_key(*k))
            .map(|(k, c)| (k.clone(), c.clone()))
            .collect();
        for (key, change) in done {
            let result = restore(system, &key, &change);
            if result == Err(ColorError::Gone) {
                // Tried again once the display is back, or on the next start.
                continue;
            }
            self.changed.remove(&key);
            system.save(&self.record());
            let original = change.original.as_deref().unwrap_or("none");
            log.push(match result {
                Ok(()) => format!("{} color profile put back: {original}.", key.1.label()),
                Err(error) => format!(
                    "Could not put back the {} color profile {original}: {error}",
                    key.1.label()
                ),
            });
        }
        for (key, (profile, game)) in wanted {
            if let Some(line) = self.apply(system, key, &profile, &game) {
                log.push(line);
            }
        }
        log
    }

    fn apply(
        &mut self,
        system: &mut impl ColorSystem,
        key: Key,
        profile: &str,
        game: &str,
    ) -> Option<String> {
        let mode = key.1;
        let previous = self.changed.get(&key).cloned();
        if previous.as_ref().is_some_and(|c| c.applied == profile) {
            return None;
        }
        let original = match &previous {
            Some(change) => change.original.clone(),
            None => match system.default_profile(&key.0, mode) {
                Ok(current) if current.as_deref() == Some(profile) => return None,
                Ok(current) => current,
                Err(ColorError::Gone) => return None,
                Err(error) => {
                    return Some(format!(
                        "Could not read the {} color profile for {game}: {error}",
                        mode.label()
                    ));
                }
            },
        };
        // Recorded before the change, as not added: after a crash in
        // between, the earlier default comes back and nothing of the
        // user's is taken off the list.
        let mut change = Change {
            display: key.0.as_str().to_owned(),
            mode,
            original,
            applied: profile.to_owned(),
            added: false,
        };
        self.changed.insert(key.clone(), change.clone());
        system.save(&self.record());
        match system.use_profile(&key.0, mode, profile) {
            Ok(added) => {
                // The profile of the game it replaces, if Huedra added it.
                if let Some(previous) = previous.filter(|p| p.added) {
                    let _ = system.remove_profile(&key.0, mode, &previous.applied);
                }
                change.added = added;
                self.changed.insert(key, change);
                system.save(&self.record());
                Some(format!(
                    "{} color profile {profile} in use for {game}.",
                    mode.label()
                ))
            }
            Err(error) => {
                match previous {
                    Some(previous) => self.changed.insert(key, previous),
                    None => self.changed.remove(&key),
                };
                system.save(&self.record());
                Some(format!(
                    "Could not use the {} color profile {profile} for {game}: {error}",
                    mode.label()
                ))
            }
        }
    }
}

/// Puts one earlier default back and takes off the profile Huedra added. A
/// list that had no default was empty, so the profile is taken off then too.
fn restore(system: &mut impl ColorSystem, key: &Key, change: &Change) -> Result<(), ColorError> {
    if let Some(original) = &change.original {
        system.use_profile(&key.0, key.1, original)?;
    }
    if change.added || change.original.is_none() {
        system.remove_profile(&key.0, key.1, &change.applied)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Display lists in memory: (display, mode) to (list, default).
    #[derive(Default)]
    struct Fake {
        lists: BTreeMap<Key, (Vec<String>, Option<String>)>,
        saved: Vec<Vec<Change>>,
        gone: BTreeSet<DisplayId>,
        refuse: Option<String>,
    }

    impl Fake {
        fn with(display: &str, mode: ColorMode, list: &[&str], default: Option<&str>) -> Self {
            let mut fake = Self::default();
            fake.set(display, mode, list, default);
            fake
        }

        fn set(&mut self, display: &str, mode: ColorMode, list: &[&str], default: Option<&str>) {
            self.lists.insert(
                (DisplayId::new(display), mode),
                (names(list), default.map(str::to_owned)),
            );
        }

        fn state(&self, display: &str, mode: ColorMode) -> (Vec<String>, Option<String>) {
            self.lists
                .get(&(DisplayId::new(display), mode))
                .cloned()
                .unwrap_or_default()
        }

        fn default_of(&self, display: &str, mode: ColorMode) -> Option<String> {
            self.state(display, mode).1
        }
    }

    impl ColorSystem for Fake {
        fn default_profile(
            &mut self,
            display: &DisplayId,
            mode: ColorMode,
        ) -> Result<Option<String>, ColorError> {
            if self.gone.contains(display) {
                return Err(ColorError::Gone);
            }
            Ok(self.default_of(display.as_str(), mode))
        }

        fn use_profile(
            &mut self,
            display: &DisplayId,
            mode: ColorMode,
            profile: &str,
        ) -> Result<bool, ColorError> {
            if self.gone.contains(display) {
                return Err(ColorError::Gone);
            }
            if self.refuse.as_deref() == Some(profile) {
                return Err(ColorError::Failed("refused".into()));
            }
            let entry = self.lists.entry((display.clone(), mode)).or_default();
            let added = !entry.0.iter().any(|p| p == profile);
            if added {
                entry.0.push(profile.to_owned());
            }
            entry.1 = Some(profile.to_owned());
            Ok(added)
        }

        fn remove_profile(
            &mut self,
            display: &DisplayId,
            mode: ColorMode,
            profile: &str,
        ) -> Result<(), ColorError> {
            if self.gone.contains(display) {
                return Err(ColorError::Gone);
            }
            let entry = self.lists.entry((display.clone(), mode)).or_default();
            entry.0.retain(|p| p != profile);
            if entry.1.as_deref() == Some(profile) {
                entry.1 = entry.0.first().cloned();
            }
            Ok(())
        }

        fn save(&mut self, record: &[Change]) {
            self.saved.push(record.to_vec());
        }
    }

    fn game(id: &str, displays: &[&str], sdr: Option<&str>, hdr: Option<&str>) -> GameColor {
        GameColor {
            id: TriggerId::new(id),
            label: id.to_owned(),
            displays: displays.iter().map(|d| DisplayId::new(*d)).collect(),
            profiles: GameProfiles {
                sdr: sdr.map(str::to_owned),
                hdr: hdr.map(str::to_owned),
            },
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn uses_and_puts_back_a_listed_profile() {
        let mut fake = Fake::with("d1", ColorMode::Hdr, &["a.icc", "b.icc"], Some("a.icc"));
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], None, Some("b.icc"))]);
        let log = state.sync(&mut fake);
        assert_eq!(log.len(), 1, "one change: {log:?}");
        assert_eq!(
            fake.state("d1", ColorMode::Hdr),
            (names(&["a.icc", "b.icc"]), Some("b.icc".into())),
            "game's profile in use"
        );
        assert!(state.is_active(), "change waits to be undone");
        state.set_games(Vec::new());
        state.sync(&mut fake);
        assert_eq!(
            fake.state("d1", ColorMode::Hdr),
            (names(&["a.icc", "b.icc"]), Some("a.icc".into())),
            "earlier default back, list unchanged"
        );
        assert!(!state.is_active(), "nothing left");
        assert_eq!(fake.saved.last(), Some(&Vec::new()), "record cleared");
    }

    #[test]
    fn takes_off_a_profile_it_added() {
        let mut fake = Fake::with("d1", ColorMode::Sdr, &["a.icm"], Some("a.icm"));
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], Some("new.icm"), None)]);
        state.sync(&mut fake);
        assert_eq!(
            fake.default_of("d1", ColorMode::Sdr).as_deref(),
            Some("new.icm"),
            "in use"
        );
        state.set_games(Vec::new());
        state.sync(&mut fake);
        assert_eq!(
            fake.state("d1", ColorMode::Sdr),
            (names(&["a.icm"]), Some("a.icm".into())),
            "added profile taken off again"
        );
    }

    #[test]
    fn empty_list_is_emptied_again() {
        let mut fake = Fake::default();
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], Some("x.icm"), None)]);
        state.sync(&mut fake);
        state.set_games(Vec::new());
        state.sync(&mut fake);
        assert_eq!(
            fake.state("d1", ColorMode::Sdr),
            (Vec::new(), None),
            "no profile, as before"
        );
    }

    #[test]
    fn nothing_to_do_when_already_in_use() {
        let mut fake = Fake::with("d1", ColorMode::Hdr, &["a.icc"], Some("a.icc"));
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], None, Some("a.icc"))]);
        assert!(state.sync(&mut fake).is_empty(), "no change, no log");
        assert!(fake.saved.is_empty(), "no record written");
        assert!(state.is_active(), "the game still counts");
    }

    #[test]
    fn the_later_game_wins_and_the_first_comes_back() {
        let mut fake = Fake::with("d1", ColorMode::Hdr, &["a.icc", "b.icc"], Some("a.icc"));
        let mut state = ColorState::default();
        let first = game("one", &["d1"], None, Some("b.icc"));
        let second = game("two", &["d1"], None, Some("c.icc"));
        state.set_games(vec![first.clone(), second]);
        state.sync(&mut fake);
        assert_eq!(
            fake.default_of("d1", ColorMode::Hdr).as_deref(),
            Some("c.icc"),
            "later game"
        );
        state.set_games(vec![first]);
        state.sync(&mut fake);
        assert_eq!(
            fake.state("d1", ColorMode::Hdr),
            (names(&["a.icc", "b.icc"]), Some("b.icc".into())),
            "first game's profile, the second one's taken off"
        );
        state.set_games(Vec::new());
        state.sync(&mut fake);
        assert_eq!(
            fake.state("d1", ColorMode::Hdr),
            (names(&["a.icc", "b.icc"]), Some("a.icc".into())),
            "the original, not the first game's"
        );
    }

    #[test]
    fn moving_to_another_display_puts_the_first_back() {
        let mut fake = Fake::with("d1", ColorMode::Sdr, &["a.icm"], Some("a.icm"));
        fake.set("d2", ColorMode::Sdr, &["z.icm"], Some("z.icm"));
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], Some("a2.icm"), None)]);
        state.sync(&mut fake);
        state.set_games(vec![game("g", &["d2"], Some("a2.icm"), None)]);
        state.sync(&mut fake);
        assert_eq!(
            fake.state("d1", ColorMode::Sdr),
            (names(&["a.icm"]), Some("a.icm".into())),
            "left display restored"
        );
        assert_eq!(
            fake.default_of("d2", ColorMode::Sdr).as_deref(),
            Some("a2.icm"),
            "new display"
        );
    }

    #[test]
    fn record_survives_a_crash() {
        let mut fake = Fake::with("d1", ColorMode::Hdr, &["a.icc", "b.icc"], Some("a.icc"));
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], None, Some("b.icc"))]);
        state.sync(&mut fake);
        let record = fake.saved.last().cloned().unwrap_or_default();
        assert_eq!(record.len(), 1, "one change recorded");
        assert_eq!(
            record[0].original.as_deref(),
            Some("a.icc"),
            "original kept"
        );
        let mut next_run = ColorState::from_record(record);
        assert!(next_run.is_active(), "work left from the last run");
        next_run.sync(&mut fake);
        assert_eq!(
            fake.default_of("d1", ColorMode::Hdr).as_deref(),
            Some("a.icc"),
            "restored on the next start"
        );
    }

    #[test]
    fn a_disconnected_display_waits() {
        let mut fake = Fake::with("d1", ColorMode::Hdr, &["a.icc", "b.icc"], Some("a.icc"));
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], None, Some("b.icc"))]);
        state.sync(&mut fake);
        fake.gone.insert(DisplayId::new("d1"));
        state.set_games(Vec::new());
        state.sync(&mut fake);
        assert!(state.is_active(), "kept for later");
        fake.gone.clear();
        state.sync(&mut fake);
        assert!(!state.is_active(), "done once it is back");
        assert_eq!(
            fake.default_of("d1", ColorMode::Hdr).as_deref(),
            Some("a.icc"),
            "restored"
        );
    }

    #[test]
    fn a_refused_profile_leaves_nothing_recorded() {
        let mut fake = Fake::with("d1", ColorMode::Hdr, &["a.icc"], Some("a.icc"));
        fake.refuse = Some("bad.icc".into());
        let mut state = ColorState::default();
        state.set_games(vec![game("g", &["d1"], None, Some("bad.icc"))]);
        let log = state.sync(&mut fake);
        assert!(
            log.first().is_some_and(|l| l.starts_with("Could not use")),
            "logged: {log:?}"
        );
        assert_eq!(fake.saved.last(), Some(&Vec::new()), "record cleared");
        assert_eq!(
            fake.default_of("d1", ColorMode::Hdr).as_deref(),
            Some("a.icc"),
            "unchanged"
        );
    }

    #[test]
    fn record_round_trip() {
        let dir = std::env::temp_dir().join(format!("huedra-test-color-{}", std::process::id()));
        let path = dir.join("color.toml");
        assert!(load_record(&path).is_empty(), "missing record");
        let changes = vec![
            Change {
                display: r"\\?\DISPLAY#ABC1234".into(),
                mode: ColorMode::Hdr,
                original: Some("HDR Calibrated Profile.icc".into()),
                applied: "Game.icc".into(),
                added: true,
            },
            Change {
                display: r"\\?\DISPLAY#ABC1234".into(),
                mode: ColorMode::Sdr,
                original: None,
                applied: "sRGB.icm".into(),
                added: false,
            },
        ];
        save_record(&path, &changes).expect("save");
        assert_eq!(load_record(&path), changes, "round trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_names() {
        assert!(is_profile_name("HDR Calibrated Profile 1.icc"), "plain");
        assert!(is_profile_name("sRGB.ICM"), "upper case");
        assert!(!is_profile_name(r"..\..\evil.icc"), "folder");
        assert!(!is_profile_name("C:x.icc"), "drive");
        assert!(!is_profile_name("notes.txt"), "kind");
        assert!(!is_profile_name(" "), "blank");
        assert!(
            !is_profile_name(&format!("{}.icc", "a".repeat(300))),
            "long"
        );
    }

    fn header(class: &[u8; 4], tags: &[&[u8; 4]]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 132 + tags.len() * 12];
        bytes[12..16].copy_from_slice(class);
        bytes[36..40].copy_from_slice(b"acsp");
        let count = u32::try_from(tags.len()).unwrap_or(0);
        bytes[128..132].copy_from_slice(&count.to_be_bytes());
        for (i, tag) in tags.iter().enumerate() {
            bytes[132 + i * 12..136 + i * 12].copy_from_slice(*tag);
        }
        bytes
    }

    #[test]
    fn inspects_profiles() {
        assert_eq!(
            inspect(&header(b"mntr", &[b"desc", b"MHC2"])),
            Some(ProfileKind {
                display: true,
                hdr: true
            }),
            "HDR calibration"
        );
        assert_eq!(
            inspect(&header(b"mntr", &[b"desc", b"vcgt"])),
            Some(ProfileKind {
                display: true,
                hdr: false
            }),
            "SDR display profile"
        );
        assert_eq!(
            inspect(&header(b"prtr", &[b"desc"])).map(|k| k.display),
            Some(false),
            "printer"
        );
        let mut cut = header(b"mntr", &[b"desc", b"MHC2"]);
        cut.truncate(140);
        assert_eq!(
            inspect(&cut).map(|k| k.hdr),
            Some(false),
            "tag table cut short"
        );
        assert_eq!(inspect(b"not a profile"), None, "too short");
        assert_eq!(inspect(&[0_u8; 200]), None, "no signature");
    }

    #[test]
    fn installed_lists_are_sorted() {
        let installed = Installed {
            sdr: names(&["b.icm", "A.icm", "a.ICM"]),
            hdr: names(&["z.icc", "m.icc"]),
        }
        .sorted();
        assert_eq!(
            installed.sdr,
            names(&["A.icm", "b.icm"]),
            "sorted, deduplicated"
        );
        assert_eq!(installed.hdr, names(&["m.icc", "z.icc"]), "sorted");
    }
}
