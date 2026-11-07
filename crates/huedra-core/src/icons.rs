//! Game icons for the settings window: Discord's public list of games has a
//! square icon for most games, matched here by store id or name; otherwise
//! the icon inside the game's own program is used. PCGamingWiki only has box
//! art, which does not fit a list. Feature doc: `docs/features/13-game-info.md`.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::files;
use crate::library::{Game, GameId, is_helper_program};

/// Host of Discord's list of games.
pub const LIST_HOST: &str = "discord.com";
/// Path of Discord's list of games (public, no account needed).
pub const LIST_PATH: &str = "/api/v9/applications/detectable";
/// The list is about 13 MB; anything far larger is refused.
pub const MAX_LIST_BYTES: usize = 32 << 20;
/// Host of the icon images.
pub const ICON_HOST: &str = "cdn.discordapp.com";
/// An icon is a few kilobytes.
pub const MAX_ICON_BYTES: usize = 256 << 10;
/// Width and height of the icons kept, in pixels.
pub const SIZE: u32 = 64;
/// Icons are looked for again after 30 days, like game information.
pub const MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;

const MAX_INDEX_BYTES: u64 = 1 << 20;
/// Folder levels searched for a game's program, and entries looked at at most.
const SEARCH_DEPTH: usize = 5;
const SEARCH_LIMIT: usize = 20_000;

#[derive(Deserialize)]
struct Entry {
    id: String,
    name: String,
    #[serde(default)]
    icon_hash: Option<String>,
    #[serde(default)]
    executables: Vec<Executable>,
    #[serde(default)]
    third_party_skus: Vec<Sku>,
}

#[derive(Deserialize)]
struct Executable {
    #[serde(default)]
    os: String,
    name: String,
}

#[derive(Deserialize)]
struct Sku {
    #[serde(default)]
    distributor: String,
    #[serde(default)]
    id: Option<String>,
}

/// One game in Discord's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    id: String,
    icon_hash: Option<String>,
    programs: Vec<String>,
}

impl App {
    /// The icon's path on [`ICON_HOST`], when the game has one and the
    /// values are well formed (digits, and a hexadecimal hash).
    pub fn icon_path(&self) -> Option<String> {
        let hash = self.icon_hash.as_deref()?;
        let id_ok = !self.id.is_empty() && self.id.bytes().all(|b| b.is_ascii_digit());
        let hash_ok = !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit());
        (id_ok && hash_ok).then(|| format!("/app-icons/{}/{hash}.png?size={SIZE}", self.id))
    }

    /// The game's Windows programs, as paths relative to its folder in lower
    /// case with `/` separators, for example `win64/deadlock.exe`.
    pub fn programs(&self) -> &[String] {
        &self.programs
    }
}

/// Discord's list, reduced to what matching needs.
#[derive(Debug, Default)]
pub struct Catalog {
    apps: Vec<App>,
    by_steam: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
}

impl Catalog {
    /// Reads the list; `None` when it is not the expected JSON.
    pub fn parse(json: &[u8]) -> Option<Self> {
        let entries: Vec<Entry> = serde_json::from_slice(json).ok()?;
        let mut catalog = Self::default();
        for entry in entries {
            let index = catalog.apps.len();
            for sku in &entry.third_party_skus {
                if sku.distributor == "steam"
                    && let Some(id) = &sku.id
                {
                    catalog.by_steam.entry(id.clone()).or_insert(index);
                }
            }
            catalog
                .by_name
                .entry(name_key(&entry.name))
                .or_insert(index);
            catalog.apps.push(App {
                id: entry.id,
                icon_hash: entry.icon_hash,
                programs: entry
                    .executables
                    .into_iter()
                    .filter(|e| e.os == "win32")
                    .map(|e| e.name.to_ascii_lowercase().replace('\\', "/"))
                    .collect(),
            });
        }
        Some(catalog)
    }

    /// The entry for a game: by Steam id, then by its program's path, then
    /// by exactly the same name (ignoring case, spaces and punctuation).
    pub fn find(&self, game: &Game) -> Option<&App> {
        if let Some(app_id) = game.id.steam_app_id()
            && let Some(&i) = self.by_steam.get(&app_id.to_string())
        {
            return self.apps.get(i);
        }
        if let Some(exe) = &game.exe {
            let exe = exe.to_ascii_lowercase().replace('\\', "/");
            // Only paths with a folder: a bare "game.exe" names many games.
            let found = self.apps.iter().find(|app| {
                app.programs
                    .iter()
                    .any(|p| p.contains('/') && exe.ends_with(&format!("/{p}")))
            });
            if found.is_some() {
                return found;
            }
        }
        let key = name_key(&game.name);
        if key.is_empty() {
            return None;
        }
        self.by_name.get(&key).and_then(|&i| self.apps.get(i))
    }
}

/// A name reduced to lower-case letters and digits, so "HADES II" and
/// "Hades II™" compare equal.
fn name_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Where a game's icon came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IconSource {
    /// Discord's list.
    Discord,
    /// The icon Steam keeps for the game.
    Steam,
    /// The game's own files: an Xbox game's package logo or the icon in
    /// its program.
    Program,
    /// None was found.
    None,
}

/// What was found for one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconEntry {
    /// The game.
    pub id: GameId,
    /// Where the icon came from.
    pub source: IconSource,
    /// Whether Discord's list was consulted; when it was not (lookups off,
    /// or no network), the game is looked at again next time.
    pub online: bool,
    /// When, in seconds since 1970.
    pub checked: u64,
}

impl IconEntry {
    /// Whether the entry still holds: younger than [`MAX_AGE_SECS`], and made
    /// with the list when the list may be used now.
    pub fn is_current(&self, now: u64, online: bool) -> bool {
        now.saturating_sub(self.checked) <= MAX_AGE_SECS && (self.online || !online)
    }
}

/// `icons/index.toml` in the local app-data folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconIndex {
    #[serde(default, rename = "game")]
    entries: Vec<IconEntry>,
}

impl IconIndex {
    /// Reads the index; a missing or damaged file gives an empty one.
    pub fn load(path: &Path) -> Self {
        files::read_limited(path, MAX_INDEX_BYTES)
            .ok()
            .flatten()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the index atomically.
    ///
    /// # Errors
    /// I/O errors from writing the file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        files::write_atomic(path, text.as_bytes())
    }

    /// The entry for a game.
    pub fn get(&self, id: &GameId) -> Option<&IconEntry> {
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Adds or replaces the entry for its game.
    pub fn put(&mut self, entry: IconEntry) {
        self.entries.retain(|e| e.id != entry.id);
        self.entries.push(entry);
    }
}

/// The icon file's name for a game: a hash of its id, so any id gives a
/// safe file name.
pub fn file_name(id: &GameId) -> String {
    // FNV-1a, 64 bits.
    let hash = id.as_str().bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}.png")
}

/// The icon Steam keeps for a game in its library cache: a small JPEG named
/// by its hash in `appcache\librarycache\<app id>\`, or
/// `<app id>_icon.jpg` in older versions of Steam.
pub fn steam_cached_icon(steam_root: &Path, app_id: u32) -> Option<PathBuf> {
    let cache = steam_root.join("appcache").join("librarycache");
    let folder = cache.join(app_id.to_string());
    let hashed = fs::read_dir(&folder).ok().and_then(|entries| {
        entries.flatten().map(|e| e.path()).find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    name.strip_suffix(".jpg").is_some_and(|hash| {
                        hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit())
                    })
                })
        })
    });
    hashed.or_else(|| {
        let old = cache.join(format!("{app_id}_icon.jpg"));
        old.is_file().then_some(old)
    })
}

/// Largest cover file read: Steam's 600 x 900 library images are about
/// 100 KB.
pub const MAX_COVER_BYTES: u64 = 2 << 20;

/// The portrait art Steam keeps for a game in its library cache:
/// `appcache\librarycache\<app id>\library_600x900.jpg`, or
/// `<app id>_library_600x900.jpg` in older versions of Steam.
pub fn steam_cached_cover(steam_root: &Path, app_id: u32) -> Option<PathBuf> {
    let cache = steam_root.join("appcache").join("librarycache");
    [
        cache.join(app_id.to_string()).join("library_600x900.jpg"),
        cache.join(format!("{app_id}_library_600x900.jpg")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// Shrinks RGBA pixels `width` x `height` by a whole `factor`, averaging
/// each block, so a 600 x 900 cover becomes 200 x 300 for the grid. Returns
/// the pixels and the new size; `None` for a factor of 0 or pixels that do
/// not match the size.
pub fn shrink(rgba: &[u8], width: u32, height: u32, factor: u32) -> Option<(Vec<u8>, u32, u32)> {
    let (w, h, f) = (
        usize::try_from(width).ok()?,
        usize::try_from(height).ok()?,
        usize::try_from(factor).ok()?,
    );
    if f == 0 || rgba.len() != w.checked_mul(h)?.checked_mul(4)? {
        return None;
    }
    let (nw, nh) = (w / f, h / f);
    let mut out = Vec::with_capacity(nw * nh * 4);
    for y in 0..nh {
        for x in 0..nw {
            let mut sum = [0_u32; 4];
            for dy in 0..f {
                let row = (y * f + dy) * w;
                for dx in 0..f {
                    let at = (row + x * f + dx) * 4;
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += u32::from(rgba[at + c]);
                    }
                }
            }
            let n = u32::try_from(f * f).ok()?;
            out.extend(sum.iter().map(|s| u8::try_from(s / n).unwrap_or(u8::MAX)));
        }
    }
    Some((out, u32::try_from(nw).ok()?, u32::try_from(nh).ok()?))
}

/// The game's main program in its folder: one of `hints` (from Discord's
/// list) when present; otherwise, among the programs that are not
/// installers, uninstallers or crash reporters, the largest whose name
/// matches the game's `name`, else the largest of all. The folder is searched
/// level by level, five levels deep, looking at no more than 20 000 entries.
pub fn find_program(folder: &Path, hints: &[String], name: &str) -> Option<PathBuf> {
    let programs = collect_programs(folder);
    let hinted = hints.iter().find_map(|hint| {
        programs
            .iter()
            .find(|p| p.relative == *hint || p.relative.ends_with(&format!("/{hint}")))
    });
    if let Some(program) = hinted {
        return Some(program.path.clone());
    }
    let game = name_key(name);
    let candidates: Vec<&Program> = programs
        .iter()
        .filter(|p| !is_helper_program(p.file_name()))
        .collect();
    let matching = candidates.iter().filter(|p| {
        let stem = name_key(p.file_name().trim_end_matches(".exe"));
        stem.len() >= 3 && game.len() >= 3 && (game.contains(&stem) || stem.contains(&game))
    });
    matching
        .max_by_key(|p| p.size)
        .or_else(|| candidates.iter().max_by_key(|p| p.size))
        .map(|p| p.path.clone())
}

/// A program found in a game's folder.
struct Program {
    /// Path from the game's folder, lower case with `/` separators.
    relative: String,
    size: u64,
    path: PathBuf,
}

impl Program {
    fn file_name(&self) -> &str {
        self.relative.rsplit('/').next().unwrap_or(&self.relative)
    }
}

fn collect_programs(folder: &Path) -> Vec<Program> {
    let mut found = Vec::new();
    let mut queue = VecDeque::from([(folder.to_path_buf(), String::new(), 0_usize)]);
    let mut seen = 0_usize;
    while let Some((dir, prefix, depth)) = queue.pop_front() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > SEARCH_LIMIT {
                return found;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && depth + 1 < SEARCH_DEPTH {
                queue.push_back((entry.path(), relative, depth + 1));
            } else if kind.is_file() && name.ends_with(".exe") {
                let size = entry.metadata().map_or(0, |m| m.len());
                found.push(Program {
                    relative,
                    size,
                    path: entry.path(),
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    #[test]
    fn shrinks_covers() {
        // 2 x 2 pixels: black, white, white, black.
        let px = [
            0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
        ];
        assert_eq!(
            super::shrink(&px, 2, 2, 2),
            Some((vec![127, 127, 127, 255], 1, 1)),
            "averaged"
        );
        assert_eq!(
            super::shrink(&px, 2, 2, 1).map(|s| s.1),
            Some(2),
            "factor 1"
        );
        assert_eq!(super::shrink(&px, 3, 2, 2), None, "size mismatch");
        assert_eq!(super::shrink(&px, 2, 2, 0), None, "factor 0");
    }

    use super::*;
    use crate::library::Store;

    const LIST: &str = r#"[
        {"id": "363430002181668864", "name": "Cuphead", "icon_hash": "e473174ecd2bf29d56541591ed2d8c31",
         "executables": [{"os": "win32", "name": "cuphead.exe"}],
         "third_party_skus": [{"distributor": "steam", "id": "268910", "sku": "268910"}],
         "extra": {"ignored": true}},
        {"id": "1276737795012165766", "name": "Deadlock",
         "executables": [{"os": "win32", "name": "win64/deadlock.exe"}, {"os": "darwin", "name": "Deadlock.app"}],
         "third_party_skus": [{"distributor": "steam", "id": "1422450"}]},
        {"id": "42", "name": "Hades II™", "icon_hash": "../../evil",
         "executables": [{"os": "win32", "name": "ship/hades2.exe"}]}
    ]"#;

    fn game(id: GameId, name: &str, exe: Option<&str>) -> Game {
        Game {
            id,
            name: name.into(),
            store: Store::Steam,
            install_dir: String::new(),
            exe: exe.map(Into::into),
        }
    }

    #[test]
    fn matches_by_steam_id_program_and_name() {
        let catalog = Catalog::parse(LIST.as_bytes()).expect("parses");
        let cuphead = game(GameId::new(Store::Steam, "268910"), "Cup head?", None);
        let app = catalog.find(&cuphead).expect("by steam id");
        assert_eq!(
            app.icon_path().as_deref(),
            Some("/app-icons/363430002181668864/e473174ecd2bf29d56541591ed2d8c31.png?size=64"),
            "icon path"
        );
        let deadlock = game(GameId::new(Store::Steam, "1422450"), "Deadlock", None);
        let app = catalog.find(&deadlock).expect("found");
        assert_eq!(app.icon_path(), None, "no icon");
        assert_eq!(
            app.programs(),
            ["win64/deadlock.exe"],
            "windows programs only"
        );

        let by_exe = game(
            GameId::new(Store::Xbox, "x"),
            "Something else",
            Some(r"C:\XboxGames\Hades\SHIP\Hades2.exe"),
        );
        assert_eq!(
            catalog.find(&by_exe).map(|a| a.id.as_str()),
            Some("42"),
            "by program path"
        );
        let by_name = game(GameId::new(Store::Epic, "x"), "HADES II", None);
        assert_eq!(
            catalog.find(&by_name).map(|a| a.id.as_str()),
            Some("42"),
            "by name"
        );
        assert_eq!(
            catalog.find(&by_name).and_then(App::icon_path),
            None,
            "bad hash refused"
        );
        let unknown = game(GameId::new(Store::Epic, "y"), "Unknown", None);
        assert!(catalog.find(&unknown).is_none(), "no match");
        assert!(
            Catalog::parse(b"{\"not\": \"a list\"}").is_none(),
            "wrong shape"
        );
    }

    #[test]
    fn index_round_trip_and_currency() {
        let dir = std::env::temp_dir().join(format!("huedra-icons-{}", std::process::id()));
        let path = dir.join("index.toml");
        let mut index = IconIndex::default();
        let entry = IconEntry {
            id: GameId::from_raw("steam:1"),
            source: IconSource::Discord,
            online: false,
            checked: 100,
        };
        index.put(entry.clone());
        index.put(entry.clone());
        index.save(&path).expect("save");
        assert_eq!(IconIndex::load(&path), index, "round trip, one entry");
        assert!(entry.is_current(100 + MAX_AGE_SECS, false), "fresh offline");
        assert!(!entry.is_current(100, true), "made offline, now online");
        assert!(!entry.is_current(101 + MAX_AGE_SECS, false), "stale");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_names_are_safe_and_stable() {
        let a = file_name(&GameId::from_raw(r"manual:c:\games\a.exe"));
        assert_eq!(a.len(), 20, "16 hex digits and .png");
        assert!(
            a.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.'),
            "safe"
        );
        assert_eq!(
            a,
            file_name(&GameId::from_raw(r"manual:c:\games\a.exe")),
            "stable"
        );
        assert_ne!(a, file_name(&GameId::from_raw("steam:1")), "distinct");
    }

    #[test]
    fn finds_steams_cached_icon() {
        let root = std::env::temp_dir().join(format!("huedra-steam-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let cache = root.join("appcache").join("librarycache");
        fs::create_dir_all(cache.join("730")).expect("dirs");
        fs::write(cache.join("730").join("library_hero.jpg"), b"x").expect("hero");
        let icon = cache
            .join("730")
            .join("8dbc71957312bbd3baea65848b545be9eae2a355.jpg");
        fs::write(&icon, b"x").expect("icon");
        fs::write(cache.join("10_icon.jpg"), b"x").expect("old layout");
        assert_eq!(steam_cached_icon(&root, 730), Some(icon), "hash-named file");
        assert_eq!(
            steam_cached_icon(&root, 10),
            Some(cache.join("10_icon.jpg")),
            "older layout"
        );
        assert_eq!(steam_cached_icon(&root, 20), None, "none");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_the_program_by_hint_or_size() {
        let dir = std::env::temp_dir().join(format!("huedra-find-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Win64")).expect("dirs");
        fs::write(dir.join("Win64").join("Game.exe"), [0_u8; 10]).expect("game");
        fs::write(dir.join("unins000.exe"), [0_u8; 500]).expect("uninstaller");
        fs::write(dir.join("Launcher.exe"), [0_u8; 100]).expect("launcher");
        fs::write(dir.join("readme.txt"), [0_u8; 900]).expect("text");
        assert_eq!(
            find_program(&dir, &["win64/game.exe".into()], "Other"),
            Some(dir.join("Win64").join("Game.exe")),
            "hint wins"
        );
        assert_eq!(
            find_program(&dir, &[], "Other"),
            Some(dir.join("Launcher.exe")),
            "largest that is not an uninstaller"
        );
        assert_eq!(
            find_program(&dir, &[], "The Game"),
            Some(dir.join("Win64").join("Game.exe")),
            "a name match wins over size"
        );
        assert_eq!(
            find_program(&dir.join("missing"), &[], "x"),
            None,
            "no folder"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
