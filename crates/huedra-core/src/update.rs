//! Updates: the release manifest, its signature, version order and the
//! record of the last check. Downloading, hashing and replacing the program
//! happen in the executable; everything that decides whether a release may
//! be installed is here and tested on any platform.
//! Feature doc: `docs/features/11-updates.md`.
//!
//! A release is three files at the release address:
//! `release.toml` (version, file name, size and SHA-256 of the program),
//! `release.toml.minisig` (its minisign signature) and the program itself.
//! Only the small manifest is signed; the program is trusted through the
//! size and digest the signed manifest gives.

use std::cmp::Ordering;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::files;

/// The public key releases are signed with, in minisign's base64 form, set
/// when the program is built (`HUEDRA_UPDATE_KEY`). Without it, and without
/// [`RELEASE_URL`], updates are off in the build.
pub const PUBLIC_KEY: Option<&str> = option_env!("HUEDRA_UPDATE_KEY");
/// Where releases are published, ending in `/`, set when the program is
/// built (`HUEDRA_UPDATE_URL`); for example
/// `https://github.com/<owner>/<repo>/releases/download/update-channel/`.
pub const RELEASE_URL: Option<&str> = option_env!("HUEDRA_UPDATE_URL");

/// Name of the signed manifest at the release address.
pub const MANIFEST_NAME: &str = "release.toml";
/// Name of the manifest's signature.
pub const SIGNATURE_NAME: &str = "release.toml.minisig";
/// Largest manifest accepted.
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024;
/// Largest signature file accepted.
pub const MAX_SIGNATURE_BYTES: usize = 2 * 1024;
/// Largest program accepted.
pub const MAX_PROGRAM_BYTES: u64 = 32 * 1024 * 1024;
/// How often the tray checks.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Largest `update.toml` read.
const MAX_STATE_BYTES: u64 = 4 * 1024;

/// The key built into this program, when there is one.
pub fn public_key() -> Option<&'static str> {
    PUBLIC_KEY.map(str::trim).filter(|k| !k.is_empty())
}

/// The release address built into this program, when there is a valid one.
pub fn release_url() -> Option<ReleaseUrl> {
    RELEASE_URL.and_then(|u| ReleaseUrl::parse(u.trim()).ok())
}

/// The project's page, for "Star on GitHub": the repository the release
/// address belongs to, when releases are published on GitHub.
pub fn project_page() -> Option<String> {
    release_url().and_then(|url| url.project_page())
}

/// Where people can support the project.
pub const SUPPORT_URL: &str = "https://ko-fi.com/pixelsthecoder";

/// Whether this build checks for updates: both the key and the address were
/// set when it was built.
pub fn is_available() -> bool {
    public_key().is_some() && release_url().is_some()
}

/// A version of three numbers, `1.2.3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version([u32; 3]);

impl Version {
    /// The version of this build. The workspace version is three numbers,
    /// which a test checks.
    pub fn current() -> Self {
        crate::VERSION.parse().unwrap_or(Self([0, 0, 0]))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl FromStr for Version {
    type Err = UpdateError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || UpdateError::BadVersion(text.to_owned());
        let mut parts = [0_u32; 3];
        let mut split = text.trim().split('.');
        for part in &mut parts {
            let piece = split.next().ok_or_else(bad)?;
            if piece.is_empty() || !piece.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            *part = piece.parse().map_err(|_| bad())?;
        }
        if split.next().is_some() {
            return Err(bad());
        }
        Ok(Self(parts))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c] = self.0;
        write!(f, "{a}.{b}.{c}")
    }
}

impl Serialize for Version {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// The release address, split for an HTTPS request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseUrl {
    host: String,
    path: String,
}

impl ReleaseUrl {
    /// Parses `https://host/path/`. Only HTTPS is accepted, without a user
    /// name, port, query or fragment; the path must end in `/`.
    ///
    /// # Errors
    /// [`UpdateError::BadUrl`] for anything else.
    pub fn parse(url: &str) -> Result<Self, UpdateError> {
        let bad = || UpdateError::BadUrl(url.to_owned());
        let rest = url.strip_prefix("https://").ok_or_else(bad)?;
        let (host, path) = rest.split_once('/').ok_or_else(bad)?;
        let host_ok = !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
        let path = format!("/{path}");
        let path_ok = path.ends_with('/')
            && !path.contains("..")
            && path
                .bytes()
                .all(|b| b.is_ascii_graphic() && !b"?#\\@".contains(&b));
        if !host_ok || !path_ok {
            return Err(bad());
        }
        Ok(Self {
            host: host.to_ascii_lowercase(),
            path,
        })
    }

    /// The host name.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The path of a file at the address.
    pub fn path_of(&self, file: &str) -> String {
        format!("{}{file}", self.path)
    }

    /// `https://github.com/<owner>/<repo>` for an address on GitHub, else
    /// `None`.
    pub fn project_page(&self) -> Option<String> {
        if self.host != "github.com" {
            return None;
        }
        let mut parts = self.path.split('/').filter(|p| !p.is_empty());
        let (owner, repo) = (parts.next()?, parts.next()?);
        Some(format!("https://github.com/{owner}/{repo}"))
    }
}

/// A release, as its signed manifest describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    version: Version,
    file: String,
    size: u64,
    sha256: [u8; 32],
}

/// `release.toml` as written by `build/pack-release.ps1`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: Version,
    file: String,
    size: u64,
    sha256: String,
}

impl Release {
    /// Checks the manifest's signature against `public_key` and reads it.
    /// Nothing in the manifest is looked at before the signature matches.
    ///
    /// # Errors
    /// [`UpdateError::Signature`] when the signature or key is not valid
    /// or does not match; [`UpdateError::Manifest`] when the signed text is
    /// not a valid manifest.
    pub fn verify(manifest: &[u8], signature: &str, public_key: &str) -> Result<Self, UpdateError> {
        if manifest.len() > MAX_MANIFEST_BYTES || signature.len() > MAX_SIGNATURE_BYTES {
            return Err(UpdateError::Manifest("the files are too large".to_owned()));
        }
        let key = minisign_verify::PublicKey::from_base64(public_key.trim())
            .map_err(|e| UpdateError::Signature(e.to_string()))?;
        let signature = minisign_verify::Signature::decode(signature)
            .map_err(|e| UpdateError::Signature(e.to_string()))?;
        key.verify(manifest, &signature, false)
            .map_err(|e| UpdateError::Signature(e.to_string()))?;
        let text =
            std::str::from_utf8(manifest).map_err(|e| UpdateError::Manifest(e.to_string()))?;
        let manifest: Manifest =
            toml::from_str(text).map_err(|e| UpdateError::Manifest(e.message().to_owned()))?;
        let file_ok = !manifest.file.is_empty()
            && manifest.file.len() <= 64
            && manifest.file.to_ascii_lowercase().ends_with(".exe")
            && !manifest.file.starts_with('.')
            && manifest
                .file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        if !file_ok {
            return Err(UpdateError::Manifest(format!(
                "`{}` is not a program name",
                manifest.file
            )));
        }
        if manifest.size == 0 || manifest.size > MAX_PROGRAM_BYTES {
            return Err(UpdateError::Manifest(format!(
                "a size of {} bytes is not allowed",
                manifest.size
            )));
        }
        let sha256 = parse_hex(&manifest.sha256)
            .ok_or_else(|| UpdateError::Manifest("the SHA-256 value is not valid".to_owned()))?;
        Ok(Self {
            version: manifest.version,
            file: manifest.file,
            size: manifest.size,
            sha256,
        })
    }

    /// The release's version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// The program's file name at the release address.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The program's size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Checks a downloaded program against the manifest.
    ///
    /// # Errors
    /// [`UpdateError::Size`] or [`UpdateError::Digest`] when it differs.
    pub fn check_program(&self, len: u64, sha256: &[u8; 32]) -> Result<(), UpdateError> {
        if len != self.size {
            return Err(UpdateError::Size {
                expected: self.size,
                found: len,
            });
        }
        if sha256 != &self.sha256 {
            return Err(UpdateError::Digest);
        }
        Ok(())
    }
}

fn parse_hex(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut out = [0_u8; 32];
    let (pairs, _) = text.as_bytes().as_chunks::<2>();
    for (byte, pair) in out.iter_mut().zip(pairs) {
        let pair = std::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

/// What a check found, given the running version and what the user chose
/// to skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The release is not newer than the running version.
    UpToDate,
    /// A newer release the user rolled back from; only installed on
    /// request.
    Skipped(Version),
    /// A newer release.
    Newer(Version),
}

/// Compares a release with the running version. Older releases are never
/// offered, so a replayed old manifest cannot downgrade Huedra.
pub fn verdict(release: Version, running: Version, skipped: Option<Version>) -> Verdict {
    if release <= running {
        Verdict::UpToDate
    } else if skipped == Some(release) {
        Verdict::Skipped(release)
    } else {
        Verdict::Newer(release)
    }
}

/// `update.toml`: when Huedra last checked, what it found, and a version
/// the user rolled back from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    /// Seconds since 1970 of the last completed check.
    pub last_check: u64,
    /// A newer version found and not yet installed (Ask before installing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<Version>,
    /// A version rolled back from; not installed automatically again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<Version>,
}

impl UpdateState {
    /// Reads the record; a missing or damaged one is empty, which only
    /// makes the next check come sooner.
    pub fn load(path: &Path) -> Self {
        files::read_limited(path, MAX_STATE_BYTES)
            .ok()
            .flatten()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the record.
    ///
    /// # Errors
    /// I/O errors from [`files::write_atomic`].
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        files::write_atomic(path, text.as_bytes())
    }

    /// How long until the next check is due at `now`: zero when it is due,
    /// and also when the clock went back past the last check.
    pub fn due_in(&self, now: SystemTime) -> Duration {
        let now = unix_secs(now);
        if self.last_check > now {
            return Duration::ZERO;
        }
        let next = self.last_check.saturating_add(CHECK_INTERVAL.as_secs());
        Duration::from_secs(next.saturating_sub(now))
    }

    /// The version waiting to be installed, if it is newer than `running`.
    pub fn waiting(&self, running: Version) -> Option<Version> {
        self.available.filter(|v| *v > running)
    }
}

/// Seconds since 1970.
pub fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// An update could not be checked or installed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// Not three numbers.
    #[error("`{0}` is not a version")]
    BadVersion(String),
    /// The release address is not an HTTPS folder address.
    #[error("`{0}` is not an HTTPS address ending in /")]
    BadUrl(String),
    /// The signature is missing, damaged, made with another key, or does
    /// not match the manifest.
    #[error("the release is not signed with Huedra's key ({0})")]
    Signature(String),
    /// The signed manifest is not valid.
    #[error("the release description is not valid: {0}")]
    Manifest(String),
    /// The downloaded program has another size.
    #[error("the downloaded program has {found} bytes instead of {expected}")]
    Size {
        /// Size in the manifest.
        expected: u64,
        /// Size received.
        found: u64,
    },
    /// The downloaded program has another SHA-256 digest.
    #[error("the downloaded program does not match its signed SHA-256 digest")]
    Digest,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Key pairs made with minisign for these tests only; their secret keys
    // were not kept.
    const TEST_KEY: &str = include_str!("../tests/data/update/test.pub");
    const OTHER_KEY: &str = include_str!("../tests/data/update/other.pub");
    const MANIFEST: &[u8] = include_bytes!("../tests/data/update/release.toml");
    const SIGNATURE: &str = include_str!("../tests/data/update/release.toml.minisig");

    /// The key line of a minisign `.pub` file.
    fn key(file: &str) -> &str {
        file.lines().last().unwrap_or_default()
    }

    #[test]
    fn versions_order_by_number() {
        let v = |s: &str| s.parse::<Version>().expect("version");
        assert!(v("0.10.0") > v("0.9.9"), "numeric, not text");
        assert!(v("1.0.0") > v("0.99.99"), "major first");
        assert_eq!(v("1.2.3").to_string(), "1.2.3", "display");
        for bad in ["1.2", "1.2.3.4", "1.x.3", "", "1..3", "-1.2.3", "+1.2.3"] {
            assert!(bad.parse::<Version>().is_err(), "{bad:?} refused");
        }
        assert_eq!(Version::current().to_string(), crate::VERSION, "current");
    }

    #[test]
    fn release_urls() {
        let url =
            ReleaseUrl::parse("https://Example.org/a/releases/latest/download/").expect("valid");
        assert_eq!(url.host(), "example.org", "host");
        assert_eq!(
            url.path_of(MANIFEST_NAME),
            "/a/releases/latest/download/release.toml",
            "path"
        );
        assert_eq!(url.project_page(), None, "not on GitHub");
        let github =
            ReleaseUrl::parse("https://github.com/owner/repo/releases/download/update-channel/")
                .expect("valid");
        assert_eq!(
            github.project_page().as_deref(),
            Some("https://github.com/owner/repo"),
            "project page"
        );
        for bad in [
            "http://example.org/a/",
            "https://example.org/a",
            "https://example.org",
            "https://user@example.org/a/",
            "https://example.org:8443/a/",
            "https://example.org/a/?x=1/",
            "https://example.org/../a/",
            "file:///C:/releases/",
        ] {
            assert!(ReleaseUrl::parse(bad).is_err(), "{bad} refused");
        }
    }

    #[test]
    fn signed_manifest_is_read() {
        let release = Release::verify(MANIFEST, SIGNATURE, key(TEST_KEY)).expect("verifies");
        assert_eq!(release.version().to_string(), "0.9.0", "version");
        assert_eq!(release.file(), "huedra.exe", "file");
        assert_eq!(release.size(), 3, "size");
        // SHA-256 of "abc".
        let abc = parse_hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .expect("hex");
        assert_eq!(release.check_program(3, &abc), Ok(()), "matching program");
        assert_eq!(
            release.check_program(4, &abc),
            Err(UpdateError::Size {
                expected: 3,
                found: 4
            }),
            "size differs"
        );
        assert_eq!(
            release.check_program(3, &[0; 32]),
            Err(UpdateError::Digest),
            "digest differs"
        );
    }

    #[test]
    fn tampering_is_refused() {
        let mut changed = MANIFEST.to_vec();
        if let Some(pos) = changed.iter().position(|b| *b == b'9') {
            changed[pos] = b'8';
        }
        assert!(
            matches!(
                Release::verify(&changed, SIGNATURE, key(TEST_KEY)),
                Err(UpdateError::Signature(_))
            ),
            "changed manifest"
        );
        assert!(
            matches!(
                Release::verify(MANIFEST, SIGNATURE, key(OTHER_KEY)),
                Err(UpdateError::Signature(_))
            ),
            "another key"
        );
        assert!(
            matches!(
                Release::verify(MANIFEST, "not a signature", key(TEST_KEY)),
                Err(UpdateError::Signature(_))
            ),
            "damaged signature"
        );
        assert!(
            matches!(
                Release::verify(MANIFEST, SIGNATURE, "RWQ"),
                Err(UpdateError::Signature(_))
            ),
            "damaged key"
        );
    }

    #[test]
    fn only_newer_releases_are_offered() {
        let v = |s: &str| s.parse::<Version>().expect("version");
        assert_eq!(
            verdict(v("0.2.0"), v("0.2.0"), None),
            Verdict::UpToDate,
            "same"
        );
        assert_eq!(
            verdict(v("0.1.0"), v("0.2.0"), None),
            Verdict::UpToDate,
            "older is never offered"
        );
        assert_eq!(
            verdict(v("0.3.0"), v("0.2.0"), None),
            Verdict::Newer(v("0.3.0")),
            "newer"
        );
        assert_eq!(
            verdict(v("0.3.0"), v("0.2.0"), Some(v("0.3.0"))),
            Verdict::Skipped(v("0.3.0")),
            "rolled back from"
        );
        assert_eq!(
            verdict(v("0.4.0"), v("0.2.0"), Some(v("0.3.0"))),
            Verdict::Newer(v("0.4.0")),
            "a later release is offered again"
        );
    }

    #[test]
    fn checks_are_daily() {
        let at = |secs: u64| UNIX_EPOCH + Duration::from_secs(secs);
        let state = UpdateState {
            last_check: 1_000_000,
            ..UpdateState::default()
        };
        assert_eq!(
            state.due_in(at(1_000_000 + 3600)),
            Duration::from_secs(23 * 3600),
            "an hour after"
        );
        assert_eq!(
            state.due_in(at(1_000_000 + 2 * 86_400)),
            Duration::ZERO,
            "overdue"
        );
        assert_eq!(state.due_in(at(10)), Duration::ZERO, "clock went back");
        assert_eq!(
            UpdateState::default().due_in(at(1_000_000)),
            Duration::ZERO,
            "never checked"
        );
    }

    #[test]
    fn state_round_trip() {
        let dir = std::env::temp_dir().join(format!("huedra-test-update-{}", std::process::id()));
        let path = dir.join("update.toml");
        assert_eq!(UpdateState::load(&path), UpdateState::default(), "missing");
        let state = UpdateState {
            last_check: 42,
            available: Some("0.3.0".parse().expect("version")),
            skipped: None,
        };
        state.save(&path).expect("save");
        assert_eq!(UpdateState::load(&path), state, "round trip");
        let running = "0.2.0".parse().expect("version");
        assert_eq!(state.waiting(running), state.available, "waiting");
        let installed = "0.3.0".parse().expect("version");
        assert_eq!(state.waiting(installed), None, "already installed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
