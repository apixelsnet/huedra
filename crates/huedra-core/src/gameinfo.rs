//! Game information from PCGamingWiki: request paths, response parsing, the
//! 30-day cache and setting suggestions. The requests themselves are made by
//! the settings process through `huedra-win`. Content is CC BY-NC-SA 3.0 and
//! is shown with attribution; only the fields used here are stored.
//! Feature doc: `docs/features/13-game-info.md`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::GameHdr;
use crate::files;
use crate::library::GameId;

/// The wiki's host.
pub const HOST: &str = "www.pcgamingwiki.com";
/// How long a result is kept before it is fetched again.
pub const MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;
/// Largest response accepted.
pub const MAX_RESPONSE_BYTES: usize = 4 << 20;
const MAX_CACHE_BYTES: u64 = 4 << 20;
const MAX_NOTES_CHARS: usize = 600;

/// What the wiki says about HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WikiHdr {
    /// Native HDR.
    Supported,
    /// HDR with limitations.
    Limited,
    /// No HDR.
    NotSupported,
    /// HDR through a modification.
    Hackable,
    /// The page does not say.
    #[default]
    Unknown,
}

impl WikiHdr {
    fn from_field(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "true" => Self::Supported,
            "limited" => Self::Limited,
            "false" => Self::NotSupported,
            "hackable" => Self::Hackable,
            _ => Self::Unknown,
        }
    }

    /// Short description for the Games page.
    pub fn label(self) -> &'static str {
        match self {
            Self::Supported => "Supports HDR",
            Self::Limited => "Limited HDR support",
            Self::NotSupported => "No HDR support",
            Self::Hackable => "HDR through a modification",
            Self::Unknown => "HDR support not recorded",
        }
    }

    /// The setting this suggests. Suggestions are shown, never applied
    /// automatically.
    pub fn suggestion(self) -> Option<GameHdr> {
        match self {
            Self::Supported | Self::Limited => Some(GameHdr::EnableWhileRunning),
            Self::NotSupported => Some(GameHdr::DoNotChange),
            Self::Hackable | Self::Unknown => None,
        }
    }
}

/// The stored result for one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInfo {
    /// The game.
    pub id: GameId,
    /// Wiki page name, `None` when no page was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// HDR support.
    #[serde(default)]
    pub hdr: WikiHdr,
    /// The page's HDR notes, with wiki markup removed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hdr_notes: String,
    /// The page's ray tracing field.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ray_tracing: String,
    /// When it was fetched, in seconds since 1970.
    pub fetched: u64,
}

impl GameInfo {
    /// The ray tracing field in plain words, or `None` when the page does
    /// not say (empty, "unknown" or "n/a").
    pub fn ray_tracing_label(&self) -> Option<&'static str> {
        match self.ray_tracing.trim().to_ascii_lowercase().as_str() {
            "true" => Some("Supported"),
            "limited" => Some("Limited support"),
            "false" => Some("Not supported"),
            "hackable" => Some("Through a modification"),
            _ => None,
        }
    }

    /// Whether the result is older than [`MAX_AGE_SECS`].
    pub fn is_stale(&self, now: u64) -> bool {
        now.saturating_sub(self.fetched) > MAX_AGE_SECS
    }

    /// The address of the wiki page, for the "Open" link.
    pub fn page_url(&self) -> Option<String> {
        self.page
            .as_ref()
            .map(|page| format!("https://{HOST}/wiki/{page}"))
    }
}

/// `gameinfo.toml` in the local app-data folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default, rename = "game")]
    entries: Vec<GameInfo>,
}

impl Cache {
    /// Reads the cache; a missing or damaged file gives an empty cache.
    pub fn load(path: &Path) -> Self {
        files::read_limited(path, MAX_CACHE_BYTES)
            .ok()
            .flatten()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the cache atomically.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        files::write_atomic(path, text.as_bytes())
    }

    /// The stored result for a game.
    pub fn get(&self, id: &GameId) -> Option<&GameInfo> {
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Stores or replaces a result.
    pub fn put(&mut self, info: GameInfo) {
        self.entries.retain(|e| e.id != info.id);
        self.entries.push(info);
    }
}

/// Path that redirects to the page of a Steam game.
pub fn appid_path(app_id: u32) -> String {
    format!("/api/appid.php?appid={app_id}")
}

/// Path that returns a page's source as JSON.
pub fn wikitext_path(page: &str) -> String {
    format!(
        "/w/api.php?action=parse&page={}&prop=wikitext&redirects=1&format=json&formatversion=2",
        encode(page)
    )
}

/// Path that searches page titles.
pub fn search_path(name: &str) -> String {
    format!(
        "/w/api.php?action=query&list=search&srsearch={}&srnamespace=0&srlimit=1&format=json&formatversion=2",
        encode(name)
    )
}

/// The page name in a wiki address such as `https://host/wiki/Cyberpunk_2077`,
/// percent-decoded.
pub fn page_from_url(url: &str) -> Option<String> {
    let page = url.split("/wiki/").nth(1)?;
    let page = page.split(['?', '#']).next()?;
    let page = decode(page);
    (!page.is_empty() && !page.starts_with("Special:")).then_some(page)
}

/// The first search result's title, as a page name.
pub fn search_result(json: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Response {
        query: Query,
    }
    #[derive(Deserialize)]
    struct Query {
        search: Vec<Hit>,
    }
    #[derive(Deserialize)]
    struct Hit {
        title: String,
    }
    let response: Response = serde_json::from_str(json).ok()?;
    let hit = response.query.search.into_iter().next()?;
    Some(hit.title.replace(' ', "_"))
}

/// The HDR fields of a `wikitext` response.
pub fn parse_wikitext(json: &str, id: GameId, page: String, now: u64) -> Option<GameInfo> {
    #[derive(Deserialize)]
    struct Response {
        parse: Parse,
    }
    #[derive(Deserialize)]
    struct Parse {
        title: String,
        wikitext: String,
    }
    let response: Response = serde_json::from_str(json).ok()?;
    let mut hdr = WikiHdr::Unknown;
    let mut hdr_notes = String::new();
    let mut ray_tracing = String::new();
    for line in response.parse.wikitext.lines() {
        let Some((key, value)) = line
            .trim()
            .strip_prefix('|')
            .and_then(|l| l.split_once('='))
        else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "hdr" if hdr == WikiHdr::Unknown => hdr = WikiHdr::from_field(value),
            "hdr notes" if hdr_notes.is_empty() => hdr_notes = clean_markup(value),
            "ray tracing" if ray_tracing.is_empty() => ray_tracing = clean_markup(value),
            _ => {}
        }
    }
    let page = if response.parse.title.is_empty() {
        page
    } else {
        response.parse.title.replace(' ', "_")
    };
    Some(GameInfo {
        id,
        page: Some(page),
        hdr,
        hdr_notes,
        ray_tracing,
        fetched: now,
    })
}

/// Removes the wiki markup that appears in notes: links, templates,
/// references, emphasis and HTML tags.
pub fn clean_markup(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("[[") {
            let end = after.find("]]").unwrap_or(after.len());
            let inner = &after[..end];
            out.push_str(inner.rsplit('|').next().unwrap_or(inner));
            rest = after.get(end + 2..).unwrap_or("");
        } else if rest.starts_with("[http") {
            // [https://example.org Label] keeps its label.
            let end = rest.find(']').unwrap_or(rest.len());
            let inner = &rest[1..end];
            out.push_str(inner.split_once(' ').map_or(inner, |(_, label)| label));
            rest = rest.get(end + 1..).unwrap_or("");
        } else if let Some(after) = rest.strip_prefix("{{") {
            let end = after.find("}}").unwrap_or(after.len());
            let inner = &after[..end];
            // {{term|text}} keeps its text; other templates are dropped.
            if let Some(term) = inner.strip_prefix("term|") {
                out.push_str(term);
            }
            rest = after.get(end + 2..).unwrap_or("");
        } else if rest.starts_with("<ref") {
            let end = rest
                .find("</ref>")
                .map(|i| i + 6)
                .or_else(|| rest.find("/>").map(|i| i + 2))
                .unwrap_or(rest.len());
            rest = &rest[end..];
        } else if c == '<' {
            let end = rest.find('>').map_or(rest.len(), |i| i + 1);
            rest = &rest[end..];
        } else if rest.starts_with("''") {
            rest = rest.trim_start_matches('\'');
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    let cleaned = out.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned.chars().take(MAX_NOTES_CHARS).collect()
}

/// Percent-encodes everything except unreserved characters.
pub fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(value)) => {
                out.push(value);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_tracing_in_plain_words() {
        let mut info = GameInfo {
            id: GameId::from_raw("steam:1"),
            page: None,
            hdr: WikiHdr::Unknown,
            hdr_notes: String::new(),
            ray_tracing: "false".into(),
            fetched: 0,
        };
        assert_eq!(info.ray_tracing_label(), Some("Not supported"), "false");
        info.ray_tracing = "True".into();
        assert_eq!(info.ray_tracing_label(), Some("Supported"), "true");
        info.ray_tracing = "unknown".into();
        assert_eq!(info.ray_tracing_label(), None, "unknown is left out");
    }

    #[test]
    fn reads_the_video_fields() {
        let json = serde_json::json!({
            "parse": {
                "title": "Cyberpunk 2077",
                "wikitext": "{{Video\n|hdr                        = true\n|hdr notes                  = Has options for [[Glossary:HDR|scRGB]] or ''PQ'' HDR10.<ref>{{Refcheck|user=X}}</ref>\n|ray tracing                = true\n}}"
            }
        })
        .to_string();
        let info =
            parse_wikitext(&json, GameId::from_raw("steam:1091500"), "x".into(), 7).expect("info");
        assert_eq!(info.hdr, WikiHdr::Supported, "hdr");
        assert_eq!(
            info.hdr_notes, "Has options for scRGB or PQ HDR10.",
            "notes cleaned"
        );
        assert_eq!(
            info.page.as_deref(),
            Some("Cyberpunk_2077"),
            "title from response"
        );
        assert_eq!(
            info.hdr.suggestion(),
            Some(GameHdr::EnableWhileRunning),
            "suggestion"
        );
        assert_eq!(
            info.page_url().as_deref(),
            Some("https://www.pcgamingwiki.com/wiki/Cyberpunk_2077"),
            "link"
        );
    }

    #[test]
    fn external_links_keep_their_label() {
        assert_eq!(
            clean_markup("[https://www.nexusmods.com/x RenoDX] fixes it."),
            "RenoDX fixes it.",
            "label kept"
        );
        assert_eq!(clean_markup("[https://a.b]"), "https://a.b", "bare link");
    }

    #[test]
    fn suggestions() {
        assert_eq!(
            WikiHdr::NotSupported.suggestion(),
            Some(GameHdr::DoNotChange),
            "no HDR"
        );
        assert_eq!(WikiHdr::Hackable.suggestion(), None, "notes only");
        assert_eq!(
            WikiHdr::from_field(" Limited "),
            WikiHdr::Limited,
            "case and space"
        );
    }

    #[test]
    fn urls_and_search() {
        assert_eq!(
            page_from_url("https://www.pcgamingwiki.com/wiki/Baldur%27s_Gate_3"),
            Some("Baldur's_Gate_3".into()),
            "decoded"
        );
        assert_eq!(
            page_from_url("https://www.pcgamingwiki.com/wiki/Special:Search"),
            None,
            "no page"
        );
        assert!(
            wikitext_path("Baldur's_Gate_3").contains("page=Baldur%27s_Gate_3"),
            "encoded"
        );
        let json = r#"{"query":{"search":[{"title":"Hades II"}]}}"#;
        assert_eq!(search_result(json), Some("Hades_II".into()), "first hit");
        assert_eq!(search_result(r#"{"query":{"search":[]}}"#), None, "no hit");
    }

    #[test]
    fn cache_round_trip_and_age() {
        let dir = std::env::temp_dir().join(format!("huedra-gameinfo-{}", std::process::id()));
        let path = dir.join("gameinfo.toml");
        let mut cache = Cache::default();
        let info = GameInfo {
            id: GameId::from_raw("steam:1"),
            page: None,
            hdr: WikiHdr::Unknown,
            hdr_notes: String::new(),
            ray_tracing: String::new(),
            fetched: 100,
        };
        cache.put(info.clone());
        cache.put(info.clone());
        cache.save(&path).expect("save");
        let loaded = Cache::load(&path);
        assert_eq!(loaded.get(&info.id), Some(&info), "round trip, one entry");
        assert!(!info.is_stale(100 + MAX_AGE_SECS), "fresh at the limit");
        assert!(info.is_stale(101 + MAX_AGE_SECS), "stale after");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
