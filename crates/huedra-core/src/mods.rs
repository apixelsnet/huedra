//! HDR mods Huedra can put into a game's folder: RenoDX (an add-on for
//! ReShade) and Luma. This module reads the two projects' lists of mods,
//! matches a game to one, reads which graphics API a program uses, and keeps
//! a record of every file an install writes, so removing the mod puts the
//! folder back as it was. It also finds mods installed by hand and takes
//! them out again, keeping a copy. Downloads and unpacking run in the
//! settings process.
//! Feature doc: `docs/features/16-hdr-mods.md`.

use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::files;
use crate::library::GameId;

/// Host of both lists of mods.
pub const LIST_HOST: &str = "raw.githubusercontent.com";
/// The RenoDX list (the project wiki's Mods page, as Markdown).
pub const RENODX_LIST: &str = "/wiki/clshortfuse/renodx/Mods.md";
/// The Luma list (the project wiki's home page, as Markdown).
pub const LUMA_LIST: &str = "/wiki/Filoppi/Luma-Framework/Home.md";
/// ReShade with add-on support, which RenoDX needs; the address always
/// leads to the newest version. It redirects through plain http, which
/// Huedra refuses, so [`reshade_setup_url`] is tried first.
pub const RESHADE_URL: &str = "https://reshade.me/downloads/ReShade_Setup_Addon.exe";

/// The newest ReShade setup with add-on support linked from ReShade's home
/// page (`downloads/ReShade_Setup_6.8.0_Addon.exe`), as an https address.
pub fn reshade_setup_url(home: &str) -> Option<String> {
    const START: &str = "downloads/ReShade_Setup_";
    const END: &str = "_Addon.exe";
    let mut rest = home;
    while let Some(i) = rest.find(START) {
        let after = &rest[i + START.len()..];
        if let Some(end) = after.find(END) {
            let version = &after[..end];
            if !version.is_empty()
                && version.len() <= 16
                && version.chars().all(|c| c.is_ascii_digit() || c == '.')
            {
                return Some(format!("https://reshade.me/{START}{version}{END}"));
            }
        }
        rest = after;
    }
    None
}
/// Largest list accepted.
pub const MAX_LIST_BYTES: usize = 4 << 20;
/// Largest download accepted (some Luma mods hold over 100 MB).
pub const MAX_DOWNLOAD_BYTES: usize = 1 << 30;
/// How long a downloaded list is used before it is fetched again.
pub const LIST_MAX_AGE_SECS: u64 = 24 * 60 * 60;
/// Files ReShade writes into the game folder once the game runs; removed
/// with the mod when they were not there before it.
pub const RUNTIME_FILES: [&str; 3] = ["ReShade.ini", "ReShade.log", "ReShadePreset.ini"];
/// Most files one mod may hold.
const MAX_FILES: usize = 4_000;

/// The project a mod comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// RenoDX, an add-on for ReShade.
    #[serde(rename = "renodx")]
    RenoDx,
    /// Luma, which brings its own ReShade.
    Luma,
}

impl Source {
    /// The project's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::RenoDx => "RenoDX",
            Self::Luma => "Luma",
        }
    }

    /// The project's list of mods, for the "Open" link.
    pub fn list_page(self) -> &'static str {
        match self {
            Self::RenoDx => "https://github.com/clshortfuse/renodx/wiki/Mods",
            Self::Luma => "https://github.com/Filoppi/Luma-Framework/wiki",
        }
    }
}

/// How far a mod is, as its list marks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Working and playable.
    Working,
    /// In progress or with known problems.
    InProgress,
}

impl Status {
    /// Text shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::InProgress => "In progress",
        }
    }
}

/// A program's processor architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    /// 64-bit.
    X64,
    /// 32-bit.
    X86,
}

/// One entry of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mod {
    /// The game's name as the list writes it.
    pub name: String,
    /// Its project.
    pub source: Source,
    /// Who makes it.
    pub author: String,
    /// How far it is.
    pub status: Status,
    /// The list's note, if any.
    pub note: String,
    /// Direct download for 64-bit games.
    pub url64: Option<String>,
    /// Direct download for 32-bit games.
    pub url32: Option<String>,
    /// A page about it (Nexus Mods or the wiki), if the list links one.
    pub page: Option<String>,
}

impl Mod {
    /// The download for a program of this architecture.
    pub fn url(&self, arch: Arch) -> Option<&str> {
        match arch {
            Arch::X64 => self.url64.as_deref(),
            Arch::X86 => self.url32.as_deref(),
        }
    }

    /// Whether Huedra can download it (the list links a file, not only a
    /// page).
    pub fn installable(&self) -> bool {
        self.url64.is_some() || self.url32.is_some()
    }
}

// ---- Lists --------------------------------------------------------------

/// The cells of a Markdown table row, or `None` for any other line.
fn cells(line: &str) -> Option<Vec<&str>> {
    let inner = line.trim().strip_prefix('|')?;
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    Some(inner.split('|').map(str::trim).collect())
}

fn is_header(first: &str) -> bool {
    first.eq_ignore_ascii_case("name") || first.starts_with(":-") || first.starts_with("--")
}

/// Every `https://` link target in a cell.
fn links(cell: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = cell;
    while let Some(i) = rest.find("](") {
        let after = &rest[i + 2..];
        let end = after.find(')').unwrap_or(after.len());
        let url = after[..end].trim();
        if url.starts_with("https://") && !url.contains(char::is_whitespace) {
            found.push(url);
        }
        rest = &after[end..];
    }
    found
}

/// The note shown when hovering a status mark: `[✅](# "note")`.
fn hover_note(cell: &str) -> String {
    cell.find("(# \"")
        .map(|i| &cell[i + 4..])
        .and_then(|rest| rest.find("\")").map(|end| plain(&rest[..end])))
        .unwrap_or_default()
}

/// A cell's text without Markdown and HTML.
fn plain(cell: &str) -> String {
    let mut text = String::new();
    let mut rest = cell;
    // A link keeps its text: [text](address).
    while let Some(open) = rest.find('[') {
        text.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match (after.find(']'), after.find("](")) {
            (Some(close), Some(link)) if close == link => {
                text.push_str(&after[..close]);
                let target = &after[close + 2..];
                rest = target.find(')').map_or("", |end| &target[end + 1..]);
            }
            _ => {
                text.push('[');
                rest = after;
            }
        }
    }
    text.push_str(rest);
    let text = text
        .replace("<br>", " ")
        .replace("<br/>", " ")
        .replace("&amp;", "&")
        .replace("&middot;", "·")
        .replace("**", "")
        .replace('`', "");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The single-game mods of the RenoDX list. Mods for whole engines (the
/// list's later sections) are left out: they need choices per game.
pub fn parse_renodx(text: &str) -> Vec<Mod> {
    let mut mods = Vec::new();
    let mut in_list = false;
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            in_list = heading.trim().eq_ignore_ascii_case("list");
            continue;
        }
        if line.starts_with("## ") {
            in_list = false;
            continue;
        }
        let Some(cells) = cells(line).filter(|c| in_list && c.len() >= 4) else {
            continue;
        };
        if is_header(cells[0]) {
            continue;
        }
        let found = links(cells[2]);
        let entry = Mod {
            name: plain(cells[0]),
            source: Source::RenoDx,
            author: plain(cells[1]),
            status: if cells[3].contains("white_check_mark") || cells[3].contains('✅') {
                Status::Working
            } else {
                Status::InProgress
            },
            note: hover_note(cells[3]),
            url64: found
                .iter()
                .find(|u| u.ends_with(".addon64"))
                .map(|u| (*u).to_owned()),
            url32: found
                .iter()
                .find(|u| u.ends_with(".addon32"))
                .map(|u| (*u).to_owned()),
            page: found
                .iter()
                .find(|u| u.contains("nexusmods.com"))
                .map(|u| (*u).to_owned()),
        };
        if !entry.name.is_empty() && (entry.installable() || entry.page.is_some()) {
            mods.push(entry);
        }
    }
    mods
}

/// The finished mods of the Luma list.
pub fn parse_luma(text: &str) -> Vec<Mod> {
    let mut mods = Vec::new();
    let mut in_list = false;
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            in_list = heading.trim().eq_ignore_ascii_case("completed mods");
            continue;
        }
        let Some(cells) = cells(line).filter(|c| in_list && c.len() >= 5) else {
            continue;
        };
        if is_header(cells[0]) {
            continue;
        }
        let found = links(cells[2]);
        let release = |u: &&&str| u.contains("/Filoppi/Luma-Framework/releases/");
        let entry = Mod {
            name: plain(cells[0]),
            source: Source::Luma,
            author: plain(cells[1]),
            status: if cells[3].contains('✅') {
                Status::Working
            } else {
                Status::InProgress
            },
            note: plain(cells[4]),
            url64: found
                .iter()
                .filter(release)
                .find(|u| u.ends_with(".zip") && !u.ends_with("-x32.zip"))
                .map(|u| (*u).to_owned()),
            url32: found
                .iter()
                .filter(release)
                .find(|u| u.ends_with("-x32.zip"))
                .map(|u| (*u).to_owned()),
            page: cells
                .get(5)
                .and_then(|c| links(c).first().map(|u| (*u).to_owned()))
                .or_else(|| {
                    found
                        .iter()
                        .find(|u| u.contains("nexusmods.com"))
                        .map(|u| (*u).to_owned())
                }),
        };
        if !entry.name.is_empty() && (entry.installable() || entry.page.is_some()) {
            mods.push(entry);
        }
    }
    mods
}

/// A name reduced for comparing: lowercase letters and digits only, "&"
/// read as "and", trademark signs dropped.
pub fn normalize(name: &str) -> String {
    name.replace('&', " and ")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The mod for a game: the list entry with the same name, preferring one
/// that works, then one Huedra can download, then RenoDX.
pub fn find<'a>(mods: &'a [Mod], game: &str) -> Option<&'a Mod> {
    let key = normalize(game);
    if key.is_empty() {
        return None;
    }
    mods.iter()
        .filter(|m| normalize(&m.name) == key)
        .min_by_key(|m| {
            (
                m.status != Status::Working,
                !m.installable(),
                m.source != Source::RenoDx,
            )
        })
}

// ---- Addresses -------------------------------------------------------------

/// The host and path of an `https://` address on a host Huedra downloads
/// mods from: reshade.me, github.com and GitHub Pages sites.
pub fn trusted(url: &str) -> Option<(&str, &str)> {
    let rest = url.strip_prefix("https://")?;
    let slash = rest.find('/')?;
    let (host, path) = rest.split_at(slash);
    let host_ok = host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let allowed = host == "reshade.me" || host == "github.com" || host.ends_with(".github.io");
    (host_ok && allowed && !path.contains(char::is_whitespace)).then_some((host, path))
}

/// The file name at the end of an address, if it is a plain file name.
pub fn file_name(url: &str) -> Option<String> {
    let name = url.rsplit('/').next()?;
    let name = name.split(['?', '#']).next()?;
    safe_relative(name).filter(|n| !n.contains('/'))
}

// ---- Programs --------------------------------------------------------------

/// What Huedra reads from a program file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// 64-bit or 32-bit.
    pub arch: Arch,
    /// The DLLs it loads at start, lowercase.
    pub imports: Vec<String>,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_at(file: &mut (impl Read + Seek), offset: u64, len: usize) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut buffer = vec![0; len];
    let mut filled = 0;
    while filled < len {
        let n = file.read(&mut buffer[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    buffer.truncate(filled);
    Ok(buffer)
}

/// Reads a Windows program's architecture and the DLLs it loads, including
/// delay-loaded ones. `None` when the file is not a program Huedra knows.
///
/// # Errors
/// I/O errors.
pub fn read_program(file: &mut (impl Read + Seek)) -> io::Result<Option<Program>> {
    let head = read_at(file, 0, 64)?;
    if head.get(..2) != Some(b"MZ".as_slice()) {
        return Ok(None);
    }
    let Some(pe) = u32_at(&head, 0x3C) else {
        return Ok(None);
    };
    let header = read_at(file, u64::from(pe), 24 + 240)?;
    if header.get(..4) != Some(b"PE\0\0".as_slice()) {
        return Ok(None);
    }
    let arch = match u16_at(&header, 4) {
        Some(0x8664) => Arch::X64,
        Some(0x014C) => Arch::X86,
        _ => return Ok(None),
    };
    let sections = usize::from(u16_at(&header, 6).unwrap_or(0)).min(96);
    let optional_size = u16_at(&header, 20).unwrap_or(0);
    let opt = 24;
    let (count_at, dirs_at) = match u16_at(&header, opt) {
        Some(0x20B) => (opt + 108, opt + 112),
        Some(0x10B) => (opt + 92, opt + 96),
        _ => return Ok(None),
    };
    let dir_count = u32_at(&header, count_at).unwrap_or(0);
    let dir = |index: u32| -> u32 {
        if index < dir_count {
            u32_at(&header, dirs_at + 8 * index as usize).unwrap_or(0)
        } else {
            0
        }
    };
    let imports_rva = dir(1);
    let delay_rva = dir(13);
    let table = read_at(
        file,
        u64::from(pe) + 24 + u64::from(optional_size),
        sections * 40,
    )?;
    let offset_of = |rva: u32| -> Option<u64> {
        table.as_chunks::<40>().0.iter().find_map(|s| {
            let size = u32_at(s, 8)?.max(u32_at(s, 16)?);
            let start = u32_at(s, 12)?;
            let raw = u32_at(s, 20)?;
            (rva >= start && rva - start < size).then(|| u64::from(raw) + u64::from(rva - start))
        })
    };
    let mut imports = Vec::new();
    // (directory, descriptor size, name offset, whether delay-loaded)
    for (rva, size, name_at, delay) in [(imports_rva, 20, 12, false), (delay_rva, 32, 4, true)] {
        let Some(start) = (rva != 0).then(|| offset_of(rva)).flatten() else {
            continue;
        };
        let descriptors = read_at(file, start, size * 256)?;
        for d in descriptors.chunks_exact(size) {
            if d.iter().all(|b| *b == 0) {
                break;
            }
            // Old delay-load tables hold addresses rather than offsets.
            if delay && u32_at(d, 0).unwrap_or(0) & 1 == 0 {
                continue;
            }
            let Some(at) = u32_at(d, name_at).and_then(offset_of) else {
                continue;
            };
            let name = read_at(file, at, 256)?;
            let end = name.iter().position(|b| *b == 0).unwrap_or(name.len());
            let name = String::from_utf8_lossy(&name[..end]).to_ascii_lowercase();
            if !name.is_empty() && !imports.contains(&name) {
                imports.push(name);
            }
        }
    }
    Ok(Some(Program { arch, imports }))
}

/// The name ReShade takes in the game folder so the game loads it: the
/// graphics API's DLL. Engines that load the API through another DLL get
/// `dxgi.dll` (Direct3D 10 to 12, by far the most common). `None` for
/// Vulkan, which ReShade reaches another way.
pub fn proxy_dll(program: &Program) -> Option<&'static str> {
    let uses = |names: &[&str]| program.imports.iter().any(|i| names.contains(&i.as_str()));
    if uses(&[
        "dxgi.dll",
        "d3d10.dll",
        "d3d10_1.dll",
        "d3d11.dll",
        "d3d12.dll",
    ]) {
        Some("dxgi.dll")
    } else if uses(&["d3d9.dll"]) {
        Some("d3d9.dll")
    } else if uses(&["opengl32.dll"]) {
        Some("opengl32.dll")
    } else if uses(&["vulkan-1.dll"]) {
        None
    } else {
        Some("dxgi.dll")
    }
}

// ---- Files -----------------------------------------------------------------

/// A relative path inside a mod with `/` between its parts, or `None` when
/// it could leave the folder (`..`, a drive, a leading slash).
pub fn safe_relative(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    if path.is_empty() || path.starts_with('/') || path.contains(':') {
        return None;
    }
    let bad = |part: &str| part.is_empty() || part == "." || part == "..";
    (!path.split('/').any(bad)).then(|| path.to_owned())
}

/// Every plain file under a folder, as safe relative paths, sorted. Links
/// and anything with an unsafe name are left out.
///
/// # Errors
/// I/O errors, or more than a mod's limit of files.
pub fn files_in(root: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    let mut folders = vec![PathBuf::new()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(root.join(&folder))? {
            let entry = entry?;
            let kind = fs::symlink_metadata(entry.path())?.file_type();
            let relative = folder.join(entry.file_name());
            if kind.is_dir() {
                folders.push(relative);
            } else if kind.is_file()
                && let Some(name) = relative.to_str().and_then(safe_relative)
            {
                found.push(name);
            }
            if found.len() + folders.len() > MAX_FILES {
                return Err(io::Error::other("the mod holds too many files"));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// One file an install put into the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    /// Relative to the game folder, with `/`.
    pub path: String,
    /// Its SHA-256 as written, in hex.
    pub sha256: String,
    /// A file of the same name was there before; it is kept in the backup
    /// folder and comes back on removal.
    #[serde(default)]
    pub replaced: bool,
}

/// The record of one installed mod, `mods\<game>.toml`. It is written
/// before the first file is copied, so an install cut short can be undone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The game.
    pub game: GameId,
    /// The game's name in the list.
    pub name: String,
    /// Its project.
    pub source: Source,
    /// Who makes it.
    #[serde(default)]
    pub author: String,
    /// Where it was downloaded from.
    pub url: String,
    /// The game folder (where the program is).
    pub folder: String,
    /// When it was installed, in seconds since 1970.
    #[serde(default)]
    pub installed: u64,
    /// Every file was copied.
    #[serde(default)]
    pub complete: bool,
    /// The files written, in order.
    #[serde(default, rename = "file")]
    pub files: Vec<Placed>,
    /// Folders the install made, outermost first.
    #[serde(default)]
    pub dirs: Vec<String>,
    /// Files of [`RUNTIME_FILES`] that were not there before.
    #[serde(default)]
    pub absent: Vec<String>,
}

/// A game id as a file name.
pub fn key(id: &GameId) -> String {
    id.as_str()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Where a game's record is kept.
pub fn manifest_path(mods: &Path, id: &GameId) -> PathBuf {
    mods.join(format!("{}.toml", key(id)))
}

/// The folder for a game's downloads and the files its mod replaced.
pub fn work_folder(mods: &Path, id: &GameId) -> PathBuf {
    mods.join(key(id))
}

fn backup_folder(mods: &Path, id: &GameId) -> PathBuf {
    work_folder(mods, id).join("backup")
}

impl Manifest {
    /// Reads a record; `None` when missing or damaged.
    pub fn load(path: &Path) -> Option<Self> {
        files::read_limited(path, 1 << 20)
            .ok()
            .flatten()
            .and_then(|text| toml::from_str(&text).ok())
    }

    /// Writes it atomically.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let text = toml::to_string(self).map_err(io::Error::other)?;
        files::write_atomic(path, text.as_bytes())
    }
}

/// Copies a mod's files into the game folder. `files` pairs each file under
/// `staged` with its place in the game folder. A file already there is
/// copied to the backup folder first. The record is saved before anything
/// is written; on an error or when `canceled` turns true, [`remove`] undoes
/// what was done.
///
/// # Errors
/// I/O errors, and [`io::ErrorKind::Interrupted`] when canceled.
pub fn place(
    mods: &Path,
    manifest: &mut Manifest,
    staged: &Path,
    files: &[(String, String)],
    hash: &dyn Fn(&[u8]) -> String,
    canceled: &dyn Fn() -> bool,
) -> io::Result<()> {
    let folder = PathBuf::from(&manifest.folder);
    let backup = backup_folder(mods, &manifest.game);
    manifest.files.clear();
    manifest.dirs.clear();
    for (from, to) in files {
        let data = fs::read(staged.join(from))?;
        manifest.files.push(Placed {
            path: to.clone(),
            sha256: hash(&data),
            replaced: folder.join(to).is_file(),
        });
        let parts: Vec<&str> = to.split('/').collect();
        let mut parent = String::new();
        for part in &parts[..parts.len() - 1] {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(part);
            if !folder.join(&parent).is_dir() && !manifest.dirs.contains(&parent) {
                manifest.dirs.push(parent.clone());
            }
        }
    }
    manifest.absent = RUNTIME_FILES
        .iter()
        .filter(|name| !folder.join(name).exists())
        .map(|name| (*name).to_owned())
        .collect();
    manifest.complete = false;
    let record = manifest_path(mods, &manifest.game);
    manifest.save(&record)?;
    for (placed, (from, _)) in manifest.files.iter().zip(files) {
        if canceled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "canceled"));
        }
        let target = folder.join(&placed.path);
        if placed.replaced {
            let saved = backup.join(&placed.path);
            if let Some(parent) = saved.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&target, &saved)?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(staged.join(from), &target)?;
    }
    manifest.complete = true;
    manifest.save(&record)
}

/// Removes a mod: deletes each file it wrote that is still as written, puts
/// back the files it replaced, deletes the ReShade files that appeared with
/// it and the folders it made once empty, then the record. Returns the
/// files left in place because they changed since (a game update, another
/// mod); their originals stay in the backup folder.
///
/// # Errors
/// I/O errors (a file in use while the game runs); running it again
/// continues where it stopped.
pub fn remove(
    mods: &Path,
    manifest: &Manifest,
    hash: &dyn Fn(&[u8]) -> String,
) -> io::Result<Vec<String>> {
    let folder = PathBuf::from(&manifest.folder);
    let backup = backup_folder(mods, &manifest.game);
    let mut kept = Vec::new();
    for placed in manifest.files.iter().rev() {
        let target = folder.join(&placed.path);
        let saved = backup.join(&placed.path);
        match fs::read(&target) {
            Ok(data) if hash(&data) == placed.sha256 => fs::remove_file(&target)?,
            Ok(data) => {
                // The original is already back (a removal that stopped
                // half way, or a copy that failed): nothing to do.
                let original = placed.replaced && fs::read(&saved).is_ok_and(|b| b == data);
                if !original {
                    kept.push(placed.path.clone());
                }
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if placed.replaced && saved.is_file() {
            fs::copy(&saved, &target)?;
        }
    }
    for name in &manifest.absent {
        let _ = fs::remove_file(folder.join(name));
    }
    for dir in manifest.dirs.iter().rev() {
        let _ = fs::remove_dir(folder.join(dir));
    }
    if kept.is_empty() {
        let _ = fs::remove_dir_all(work_folder(mods, &manifest.game));
    }
    match fs::remove_file(manifest_path(mods, &manifest.game)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    Ok(kept)
}

/// Every record in the mods folder.
pub fn installed(mods: &Path) -> Vec<Manifest> {
    let Ok(entries) = fs::read_dir(mods) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .filter_map(|e| Manifest::load(&e.path()))
        .collect()
}

// ---- Mods installed by hand ------------------------------------------------

/// How deep below a game's folder [`find_in`] looks: Unreal games keep their
/// program in `<Game>\Binaries\Win64`.
const FIND_DEPTH: usize = 4;
/// Most folder entries [`find_in`] reads per game, so a large game cannot
/// slow it down.
const FIND_ENTRIES: usize = 4_000;
/// Folders that never hold a mod's add-on and can be large.
const SKIP_FOLDERS: [&str; 6] = [
    "content",
    "paks",
    "movies",
    "reshade-shaders",
    "luma",
    "__installer",
];

/// The project of a mod's add-on file by its name: `renodx-*.addon64` or
/// `.addon32` for RenoDX, `Luma-*.addon` (or `.addon64`, `.addon32`) for
/// Luma.
pub fn add_on_source(file_name: &str) -> Option<Source> {
    let name = file_name.to_ascii_lowercase();
    let add_on = [".addon", ".addon64", ".addon32"]
        .iter()
        .any(|ext| name.ends_with(ext));
    if !add_on {
        None
    } else if name.starts_with("renodx") {
        Some(Source::RenoDx)
    } else if name.starts_with("luma") {
        Some(Source::Luma)
    } else {
        None
    }
}

/// An HDR mod found in a game's folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its project.
    pub source: Source,
    /// The folder its add-on is in.
    pub folder: PathBuf,
}

/// Looks for an HDR mod's add-on in the given folders and the folders below
/// them, a few levels deep and within a limit of entries.
pub fn find_in(folders: &[PathBuf]) -> Option<Found> {
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> =
        folders.iter().map(|f| (f.clone(), 0)).collect();
    let mut read = 0;
    while let Some((folder, depth)) = queue.pop_front() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            read += 1;
            if read > FIND_ENTRIES {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_file() {
                if let Some(source) = add_on_source(&name) {
                    return Some(Found {
                        source,
                        folder: folder.clone(),
                    });
                }
            } else if kind.is_dir()
                && depth < FIND_DEPTH
                && !name.ends_with("_data")
                && !SKIP_FOLDERS.contains(&name.as_str())
            {
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }
    None
}

// ---- Removing a mod installed by hand --------------------------------------

/// Largest DLL read to see whether it is ReShade (ReShade is about 6 MB).
const MAX_RESHADE_BYTES: u64 = 32 << 20;

/// The `ProductName` in a program's version information, read from its
/// bytes; `None` when it has none.
pub fn product_name(bytes: &[u8]) -> Option<String> {
    let key: Vec<u8> = "ProductName\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut from = 0;
    while let Some(at) = bytes
        .get(from..)?
        .windows(key.len())
        .position(|w| w == key.as_slice())
    {
        let start = from + at;
        from = start + 1;
        if start % 2 != 0 {
            continue;
        }
        // The value follows after zero padding.
        let mut i = start + key.len();
        while u16_at(bytes, i) == Some(0) && i - start - key.len() < 6 {
            i += 2;
        }
        let mut units = Vec::new();
        while let Some(unit) = u16_at(bytes, i) {
            if unit == 0 || units.len() > 64 {
                break;
            }
            units.push(unit);
            i += 2;
        }
        if !units.is_empty() {
            return String::from_utf16(&units).ok();
        }
    }
    None
}

/// Whether a file is ReShade, by the product name in its version
/// information.
pub fn is_reshade(path: &Path) -> bool {
    let small = fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_RESHADE_BYTES);
    small
        && fs::read(path)
            .ok()
            .and_then(|bytes| product_name(&bytes))
            .is_some_and(|name| name == "ReShade")
}

/// What removing a mod installed by hand takes out of the folder its
/// add-on is in. Names are relative to that folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandRemoval {
    /// The folder the add-on is in.
    pub folder: PathBuf,
    /// Files and folders to take out; ReShade's program, when it goes, first.
    pub items: Vec<String>,
    /// ReShade's program in the folder, when one was found.
    pub reshade: Option<String>,
    /// Other ReShade add-ons are in the folder, so ReShade and its files
    /// stay.
    pub reshade_kept: bool,
}

/// Works out what removing the RenoDX or Luma mod in `folder` takes out:
/// the mod's add-ons, Luma's folder, and ReShade with its settings, log and
/// shaders, unless other add-ons use ReShade. Nothing outside the folder.
///
/// # Errors
/// I/O errors reading the folder.
pub fn plan_hand_removal(folder: &Path) -> io::Result<HandRemoval> {
    let mut add_ons = Vec::new();
    let mut other_add_ons = false;
    let mut reshade = None;
    let mut luma_dir = None;
    let mut present = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(folder)?.filter_map(Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries.iter().take(MAX_FILES) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() || kind.is_symlink() {
            if lower == "luma" {
                luma_dir = Some(name);
            } else if lower == "reshade-shaders" {
                present.push(name);
            }
            continue;
        }
        if add_on_source(&lower).is_some() {
            add_ons.push(name);
        } else if [".addon", ".addon64", ".addon32"]
            .iter()
            .any(|ext| lower.ends_with(ext))
        {
            other_add_ons = true;
        } else if lower.ends_with(".dll") && reshade.is_none() && is_reshade(&entry.path()) {
            reshade = Some(name);
        } else if RUNTIME_FILES.iter().any(|f| f.eq_ignore_ascii_case(&name)) {
            present.push(name);
        }
    }
    let mut items = Vec::new();
    let reshade_goes = reshade.is_some() && !other_add_ons;
    if reshade_goes {
        items.extend(reshade.clone());
    }
    let luma = add_ons
        .iter()
        .any(|n| add_on_source(n) == Some(Source::Luma));
    items.extend(add_ons);
    if luma {
        items.extend(luma_dir);
    }
    if reshade_goes {
        items.extend(present);
    }
    Ok(HandRemoval {
        folder: folder.to_path_buf(),
        items,
        reshade_kept: reshade.is_some() && !reshade_goes,
        reshade,
    })
}

/// Where the files of a mod installed by hand are kept once removed: a new
/// folder per removal, outside the game's work folder, which a later
/// removal by Huedra deletes.
pub fn removed_folder(mods: &Path, id: &GameId, now: u64) -> PathBuf {
    mods.join("removed").join(format!("{}-{now}", key(id)))
}

/// Copies a file or folder; links are not followed, and inside a folder
/// they are skipped (their target is not the mod's).
fn copy_item(from: &Path, to: &Path) -> io::Result<()> {
    let kind = fs::symlink_metadata(from)?.file_type();
    if kind.is_symlink() {
        Ok(())
    } else if kind.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_item(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

/// Deletes a file, a folder with what is in it, or only the link when it
/// is one.
fn delete_item(path: &Path) -> io::Result<()> {
    let kind = match fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if kind.is_symlink() {
        // A link to a folder is removed as a folder on Windows.
        fs::remove_file(path).or_else(|_| fs::remove_dir(path))
    } else if kind.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// The order items are deleted in: ReShade's files and Luma's folder, then
/// ReShade's program, then the add-ons. A removal that stops part way thus
/// leaves the add-on, so the mod is still found and Remove offered, and
/// leaves ReShade's program while any of its files remain, so a new plan
/// lists them again.
fn deletion_order(removal: &HandRemoval) -> Vec<&str> {
    let rank = |item: &str| {
        if add_on_source(item).is_some() {
            2
        } else if removal.reshade.as_deref() == Some(item) {
            1
        } else {
            0
        }
    };
    let mut order: Vec<&str> = removal.items.iter().map(String::as_str).collect();
    order.sort_by_key(|item| rank(item));
    order
}

/// Takes a mod installed by hand out of its folder: copies every item to
/// `keep` first, then deletes them in the order `deletion_order` gives. Running it
/// again after an error continues with what is left.
///
/// # Errors
/// I/O errors; when copying or the first deletion fails, nothing was
/// deleted and the copy is dropped again.
pub fn remove_by_hand(removal: &HandRemoval, keep: &Path) -> io::Result<()> {
    let copied = (|| {
        fs::create_dir_all(keep)?;
        for item in &removal.items {
            let from = removal.folder.join(item);
            if fs::symlink_metadata(&from).is_ok() {
                copy_item(&from, &keep.join(item))?;
            }
        }
        Ok(())
    })();
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(keep);
        return Err(error);
    }
    for (n, item) in deletion_order(removal).into_iter().enumerate() {
        if let Err(error) = delete_item(&removal.folder.join(item)) {
            if n == 0 {
                let _ = fs::remove_dir_all(keep);
            }
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RENODX: &str = "# Installation\n\n| Name | x | y | z |\n| a | b | c | d |\n\n# List\n\n| Name | Maintainer | Links | Status |\n| :--- | :--- | :--- | :---: |\n| Hollow Knight: Silksong | ShortFuse | [![Snapshot](https://img.shields.io/x)](https://clshortfuse.github.io/renodx/renodx-hollowknight-silksong.addon64) | :white_check_mark: |\n| Alien: Isolation | Musa | [![Nexus Mods](https://img.shields.io/n)](https://www.nexusmods.com/alienisolation/mods/78) &middot; [![Snapshot](https://img.shields.io/s)](https://github.com/mqhaji/renodx/releases/download/snapshot/renodx-alienisolation.addon32) | :construction: |\n| Alice: Madness Returns | Steve | [![Snapshot](https://img.shields.io/s)](https://x.github.io/renodx/renodx-alice.addon32) | [:white_check_mark:](# \"Check the mod's thread Notes.\") |\n\n## Multi-Game Mods\n\n| Unreal | a | [x](https://a.github.io/renodx/renodx-ue.addon64) | :white_check_mark: |\n";

    const LUMA: &str = "Intro\n\n# Completed Mods\n\n| Name | Author | Download Link |   Status   | Special Notes | Features |\n|------|--------|---------------|------------|---------------|----------|\n| Final Fantasy XV | izueh | [![GitHub][gh]](https://github.com/Filoppi/Luma-Framework/releases/latest/download/Luma-Final_Fantasy_XV.zip) | ✅ | | [📌](https://github.com/Filoppi/Luma-Framework/wiki#final-fantasy-xv) |\n| Call of Duty: Black Ops III | X | [![GitHub][gh]](https://github.com/Filoppi/Luma-Framework/releases/latest/download/Luma-Call_of_Duty_Black_Ops_3.zip) | ✅ | You may get banned.<br>Avoid online modes. | |\n| BioShock Remastered | Pumbo | [![GitHub][gh]](https://github.com/Filoppi/Luma-Framework/releases/latest/download/Luma-BioShock_Series-x32.zip) | 🚧 | | |\n\n# WIP Mods\n\n| Name | Author | Status |\n| Later | x | 🚧 |\n";

    #[test]
    fn reads_the_renodx_list() {
        let mods = parse_renodx(RENODX);
        assert_eq!(mods.len(), 3, "the List section only: {mods:?}");
        assert_eq!(mods[0].name, "Hollow Knight: Silksong", "name");
        assert_eq!(mods[0].author, "ShortFuse", "author");
        assert_eq!(mods[0].status, Status::Working, "working");
        assert_eq!(
            mods[0].url64.as_deref(),
            Some("https://clshortfuse.github.io/renodx/renodx-hollowknight-silksong.addon64"),
            "64-bit download"
        );
        assert_eq!(mods[1].status, Status::InProgress, "in progress");
        assert!(mods[1].url64.is_none() && mods[1].url32.is_some(), "32-bit");
        assert_eq!(
            mods[1].page.as_deref(),
            Some("https://www.nexusmods.com/alienisolation/mods/78"),
            "Nexus page"
        );
        assert_eq!(mods[2].note, "Check the mod's thread Notes.", "hover note");
    }

    #[test]
    fn reads_the_luma_list() {
        let mods = parse_luma(LUMA);
        assert_eq!(mods.len(), 3, "completed mods only: {mods:?}");
        assert_eq!(mods[0].author, "izueh", "author");
        assert!(mods[0].url64.is_some() && mods[0].url32.is_none(), "64-bit");
        assert_eq!(
            mods[0].page.as_deref(),
            Some("https://github.com/Filoppi/Luma-Framework/wiki#final-fantasy-xv"),
            "features page"
        );
        assert_eq!(
            mods[1].note, "You may get banned. Avoid online modes.",
            "note"
        );
        assert!(mods[2].url32.is_some() && mods[2].url64.is_none(), "32-bit");
        assert_eq!(mods[2].status, Status::InProgress, "in progress");
    }

    #[test]
    fn finds_a_game() {
        let mut mods = parse_renodx(RENODX);
        mods.extend(parse_luma(LUMA));
        let found = find(&mods, "HOLLOW KNIGHT SILKSONG™");
        assert_eq!(
            found.map(|m| m.source),
            Some(Source::RenoDx),
            "name differs only in case and signs"
        );
        assert_eq!(
            find(&mods, "Final Fantasy XV").map(|m| m.source),
            Some(Source::Luma),
            "Luma"
        );
        assert!(find(&mods, "Celeste").is_none(), "no mod");
        assert!(find(&mods, "™").is_none(), "empty name");
        assert_eq!(normalize("Ratchet & Clank"), "ratchetandclank", "and");
    }

    #[test]
    fn trusts_only_known_hosts() {
        assert_eq!(
            trusted("https://clshortfuse.github.io/renodx/renodx-x.addon64"),
            Some(("clshortfuse.github.io", "/renodx/renodx-x.addon64")),
            "GitHub Pages"
        );
        assert!(trusted(RESHADE_URL).is_some(), "ReShade");
        assert_eq!(
            reshade_setup_url(
                "<a href=\"/downloads/ReShade_Setup_6.8.0.exe\">x</a><a href=\"/downloads/ReShade_Setup_6.8.0_Addon.exe\">"
            )
            .as_deref(),
            Some("https://reshade.me/downloads/ReShade_Setup_6.8.0_Addon.exe"),
            "the add-on build"
        );
        assert!(
            reshade_setup_url("downloads/ReShade_Setup_x\"><script>_Addon.exe").is_none(),
            "a version is digits and dots"
        );
        assert!(trusted("http://reshade.me/x").is_none(), "https only");
        assert!(trusted("https://evil.example/x").is_none(), "other host");
        assert!(
            trusted("https://github.io.evil.example/x").is_none(),
            "suffix"
        );
        assert_eq!(
            file_name("https://a.github.io/renodx/renodx-x.addon64").as_deref(),
            Some("renodx-x.addon64"),
            "file name"
        );
    }

    #[test]
    fn rejects_paths_that_leave_the_folder() {
        assert_eq!(
            safe_relative(r"Luma\Global\a.hlsl").as_deref(),
            Some("Luma/Global/a.hlsl"),
            "kept"
        );
        for bad in ["../x.dll", "/x.dll", r"C:\x.dll", "a/./b", "a//b", ""] {
            assert!(safe_relative(bad).is_none(), "{bad}");
        }
    }

    /// A 64-bit program that imports `D3D11.dll`.
    fn program() -> Vec<u8> {
        let mut file = vec![0_u8; 0x400];
        file[..2].copy_from_slice(b"MZ");
        file[0x3C..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
        file[0x40..0x44].copy_from_slice(b"PE\0\0");
        file[0x44..0x46].copy_from_slice(&0x8664_u16.to_le_bytes());
        file[0x46..0x48].copy_from_slice(&1_u16.to_le_bytes());
        file[0x54..0x56].copy_from_slice(&240_u16.to_le_bytes());
        let opt = 0x58;
        file[opt..opt + 2].copy_from_slice(&0x20B_u16.to_le_bytes());
        file[opt + 108..opt + 112].copy_from_slice(&16_u32.to_le_bytes());
        file[opt + 120..opt + 124].copy_from_slice(&0x1000_u32.to_le_bytes());
        let section = opt + 240;
        file[section + 8..section + 12].copy_from_slice(&0x200_u32.to_le_bytes());
        file[section + 12..section + 16].copy_from_slice(&0x1000_u32.to_le_bytes());
        file[section + 16..section + 20].copy_from_slice(&0x200_u32.to_le_bytes());
        file[section + 20..section + 24].copy_from_slice(&0x200_u32.to_le_bytes());
        file[0x200 + 12..0x200 + 16].copy_from_slice(&0x1100_u32.to_le_bytes());
        file[0x300..0x30A].copy_from_slice(b"D3D11.dll\0");
        file
    }

    #[test]
    fn reads_a_program() {
        let program = read_program(&mut io::Cursor::new(program()))
            .expect("read")
            .expect("a program");
        assert_eq!(program.arch, Arch::X64, "64-bit");
        assert_eq!(program.imports, ["d3d11.dll"], "imports");
        assert_eq!(proxy_dll(&program), Some("dxgi.dll"), "Direct3D 11");
        let nine = Program {
            arch: Arch::X86,
            imports: vec!["d3d9.dll".into()],
        };
        assert_eq!(proxy_dll(&nine), Some("d3d9.dll"), "Direct3D 9");
        let vulkan = Program {
            arch: Arch::X64,
            imports: vec!["vulkan-1.dll".into()],
        };
        assert_eq!(proxy_dll(&vulkan), None, "Vulkan");
        assert!(
            read_program(&mut io::Cursor::new(b"not a program".to_vec()))
                .expect("read")
                .is_none(),
            "not a program"
        );
    }

    fn manifest(id: &str, folder: &Path, source: Source) -> Manifest {
        Manifest {
            game: GameId::from_raw(id),
            name: "Game".into(),
            source,
            author: "A".into(),
            url: "https://github.com/x".into(),
            folder: folder.display().to_string(),
            installed: 0,
            complete: false,
            files: Vec::new(),
            dirs: Vec::new(),
            absent: Vec::new(),
        }
    }

    fn text_hash(data: &[u8]) -> String {
        String::from_utf8_lossy(data).into_owned()
    }

    #[test]
    fn installs_and_removes_without_a_trace() {
        let root = std::env::temp_dir().join(format!("huedra-mods-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let game = root.join("game");
        let staged = root.join("staged");
        let mods = root.join("mods");
        for dir in [&game, &staged.join("Luma"), &mods] {
            fs::create_dir_all(dir).expect("folders");
        }
        fs::write(game.join("Game.exe"), "game").expect("write");
        fs::write(game.join("dxgi.dll"), "original").expect("write");
        fs::write(staged.join("dxgi.dll"), "reshade").expect("write");
        fs::write(staged.join("Luma").join("a.hlsl"), "shader").expect("write");
        let mut record = manifest("steam:1", &game, Source::Luma);
        let files = files_in(&staged).expect("staged files");
        assert_eq!(files, ["Luma/a.hlsl", "dxgi.dll"], "found and sorted");
        let pairs: Vec<(String, String)> = files.iter().map(|f| (f.clone(), f.clone())).collect();
        place(&mods, &mut record, &staged, &pairs, &text_hash, &|| false).expect("place");
        assert_eq!(
            fs::read_to_string(game.join("dxgi.dll")).ok().as_deref(),
            Some("reshade"),
            "replaced"
        );
        assert!(record.complete, "complete");
        assert_eq!(record.dirs, ["Luma"], "made the folder");
        assert_eq!(installed(&mods).len(), 1, "recorded");
        fs::write(game.join("ReShade.ini"), "written by ReShade").expect("write");
        let kept = remove(&mods, &record, &text_hash).expect("remove");
        assert!(kept.is_empty(), "nothing kept: {kept:?}");
        assert_eq!(
            fs::read_to_string(game.join("dxgi.dll")).ok().as_deref(),
            Some("original"),
            "put back"
        );
        assert!(!game.join("Luma").exists(), "folder gone");
        assert!(!game.join("ReShade.ini").exists(), "ReShade's file gone");
        assert!(game.join("Game.exe").exists(), "the game stays");
        assert!(installed(&mods).is_empty(), "record gone");
        assert!(!work_folder(&mods, &record.game).exists(), "backup gone");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_mods_installed_by_hand() {
        assert_eq!(
            add_on_source("Luma-Unity Engine.addon"),
            Some(Source::Luma),
            "Luma"
        );
        assert_eq!(
            add_on_source("renodx-mhwilds.addon64"),
            Some(Source::RenoDx),
            "RenoDX"
        );
        assert_eq!(add_on_source("renodx-notes.txt"), None, "not an add-on");
        assert_eq!(add_on_source("other.addon64"), None, "another add-on");
        let root = std::env::temp_dir().join(format!("huedra-mods-found-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let win64 = root.join("Game").join("Binaries").join("Win64");
        fs::create_dir_all(&win64).expect("folders");
        fs::create_dir_all(root.join("Game").join("Content")).expect("folders");
        fs::write(
            root.join("Game").join("Content").join("renodx-x.addon64"),
            "",
        )
        .expect("write");
        assert_eq!(
            find_in(std::slice::from_ref(&root)),
            None,
            "Content is skipped"
        );
        fs::write(win64.join("renodx-game.addon64"), "").expect("write");
        assert_eq!(
            find_in(std::slice::from_ref(&root)),
            Some(Found {
                source: Source::RenoDx,
                folder: win64.clone(),
            }),
            "found three levels down"
        );
        assert_eq!(find_in(&[root.join("missing")]), None, "missing folder");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_files_that_changed() {
        let root = std::env::temp_dir().join(format!("huedra-mods-kept-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let game = root.join("game");
        let staged = root.join("staged");
        let mods = root.join("mods");
        for dir in [&game, &staged, &mods] {
            fs::create_dir_all(dir).expect("folders");
        }
        fs::write(staged.join("x.addon64"), "addon").expect("write");
        let mut record = manifest("steam:2", &game, Source::RenoDx);
        let pairs = [("x.addon64".to_owned(), "x.addon64".to_owned())];
        let canceled = place(&mods, &mut record, &staged, &pairs, &text_hash, &|| true);
        assert!(canceled.is_err(), "canceled before copying");
        assert!(!game.join("x.addon64").exists(), "nothing copied");
        place(&mods, &mut record, &staged, &pairs, &text_hash, &|| false).expect("place");
        fs::write(game.join("x.addon64"), "updated").expect("write");
        let kept = remove(&mods, &record, &text_hash).expect("remove");
        assert_eq!(kept, ["x.addon64"], "changed file kept");
        assert!(game.join("x.addon64").exists(), "still there");
        let _ = fs::remove_dir_all(&root);
    }

    /// A DLL whose version information names `product`.
    fn dll(product: &str) -> Vec<u8> {
        let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_le_bytes).collect() };
        let mut bytes = b"MZ".to_vec();
        bytes.resize(64, 0);
        bytes.extend(utf16("CompanyName\0"));
        bytes.extend(utf16("crosire\0"));
        bytes.extend(utf16("ProductName\0"));
        bytes.extend([0, 0]);
        bytes.extend(utf16(product));
        bytes.extend([0, 0, 0, 0]);
        bytes
    }

    #[test]
    fn reads_the_product_name() {
        assert_eq!(
            product_name(&dll("ReShade")).as_deref(),
            Some("ReShade"),
            "ReShade"
        );
        assert_eq!(
            product_name(&dll("Microsoft® Windows® Operating System")).as_deref(),
            Some("Microsoft® Windows® Operating System"),
            "Windows"
        );
        assert_eq!(product_name(b"MZ no version"), None, "none");
    }

    fn game_folder(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("huedra-mods-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("game")).expect("folders");
        root
    }

    #[test]
    fn removes_a_mod_installed_by_hand() {
        let root = game_folder("hand");
        let game = root.join("game");
        fs::write(game.join("dxgi.dll"), dll("ReShade")).expect("write");
        fs::write(game.join("steam_api64.dll"), dll("Steamworks")).expect("write");
        fs::write(game.join("Game.exe"), "game").expect("write");
        fs::write(game.join("renodx-game.addon64"), "addon").expect("write");
        fs::write(game.join("ReShade.ini"), "ini").expect("write");
        fs::write(game.join("ReShade.log"), "log").expect("write");
        fs::create_dir_all(game.join("reshade-shaders").join("Shaders")).expect("folders");
        fs::write(
            game.join("reshade-shaders").join("Shaders").join("a.fx"),
            "fx",
        )
        .expect("write");
        let plan = plan_hand_removal(&game).expect("plan");
        assert_eq!(
            plan.items,
            [
                "dxgi.dll",
                "renodx-game.addon64",
                "ReShade.ini",
                "ReShade.log",
                "reshade-shaders"
            ],
            "ReShade first, then the add-on and ReShade's files"
        );
        assert_eq!(plan.reshade.as_deref(), Some("dxgi.dll"), "ReShade found");
        assert!(!plan.reshade_kept, "ReShade goes");
        let keep = removed_folder(&root.join("mods"), &GameId::from_raw("steam:1"), 7);
        remove_by_hand(&plan, &keep).expect("remove");
        let mut left: Vec<String> = fs::read_dir(&game)
            .expect("read")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["Game.exe", "steam_api64.dll"],
            "only the game's files stay"
        );
        assert_eq!(
            fs::read(keep.join("reshade-shaders").join("Shaders").join("a.fx")).expect("kept"),
            b"fx",
            "copies kept"
        );
        assert!(keep.join("dxgi.dll").is_file(), "ReShade kept");
        assert_eq!(
            plan_hand_removal(&game).expect("plan").items,
            Vec::<String>::new(),
            "nothing left to remove"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_removal_that_stopped_can_be_finished() {
        let root = game_folder("stopped");
        let game = root.join("game");
        fs::write(game.join("dxgi.dll"), dll("ReShade")).expect("write");
        fs::write(game.join("renodx-game.addon64"), "addon").expect("write");
        fs::write(game.join("ReShade.ini"), "ini").expect("write");
        fs::write(game.join("ReShade.log"), "log").expect("write");
        let plan = plan_hand_removal(&game).expect("plan");
        assert_eq!(
            deletion_order(&plan),
            [
                "ReShade.ini",
                "ReShade.log",
                "dxgi.dll",
                "renodx-game.addon64"
            ],
            "ReShade's files, then ReShade, then the add-on"
        );
        // Stopped after ReShade.ini: ReShade is still there, so the rest is
        // listed again.
        fs::remove_file(game.join("ReShade.ini")).expect("delete");
        assert_eq!(
            plan_hand_removal(&game).expect("plan").items,
            ["dxgi.dll", "renodx-game.addon64", "ReShade.log"],
            "the rest"
        );
        // Stopped before the add-on: the mod is still found.
        fs::remove_file(game.join("ReShade.log")).expect("delete");
        fs::remove_file(game.join("dxgi.dll")).expect("delete");
        assert!(
            find_in(std::slice::from_ref(&game)).is_some(),
            "still found"
        );
        let plan = plan_hand_removal(&game).expect("plan");
        assert_eq!(plan.items, ["renodx-game.addon64"], "the add-on");
        remove_by_hand(&plan, &root.join("kept")).expect("remove");
        assert_eq!(fs::read_dir(&game).expect("read").count(), 0, "clean");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removes_luma_and_keeps_reshade_for_other_add_ons() {
        let root = game_folder("luma");
        let game = root.join("game");
        fs::write(game.join("dxgi.dll"), dll("ReShade")).expect("write");
        fs::write(game.join("Luma-Unity Engine.addon"), "addon").expect("write");
        fs::create_dir_all(game.join("Luma").join("Global")).expect("folders");
        fs::write(game.join("Luma").join("Global").join("a.hlsl"), "x").expect("write");
        fs::write(game.join("ReShade.ini"), "ini").expect("write");
        let plan = plan_hand_removal(&game).expect("plan");
        assert_eq!(
            plan.items,
            ["dxgi.dll", "Luma-Unity Engine.addon", "Luma", "ReShade.ini"],
            "Luma with its folder"
        );
        fs::write(game.join("other.addon64"), "another add-on").expect("write");
        let plan = plan_hand_removal(&game).expect("plan");
        assert_eq!(
            plan.items,
            ["Luma-Unity Engine.addon", "Luma"],
            "ReShade stays for the other add-on"
        );
        assert!(plan.reshade_kept, "said so");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn removes_only_the_link_to_a_shared_folder() {
        let root = game_folder("link");
        let game = root.join("game");
        let shared = root.join("shared");
        fs::create_dir_all(&shared).expect("folders");
        fs::write(shared.join("mine.fx"), "keep").expect("write");
        std::os::unix::fs::symlink(&shared, game.join("reshade-shaders")).expect("link");
        fs::write(game.join("dxgi.dll"), dll("ReShade")).expect("write");
        fs::write(game.join("renodx-game.addon64"), "addon").expect("write");
        let plan = plan_hand_removal(&game).expect("plan");
        assert!(
            plan.items.iter().any(|i| i == "reshade-shaders"),
            "link listed"
        );
        remove_by_hand(&plan, &root.join("kept")).expect("remove");
        assert!(!game.join("reshade-shaders").exists(), "link gone");
        assert!(shared.join("mine.fx").is_file(), "its target untouched");
        let _ = fs::remove_dir_all(&root);
    }
}
