//! Video players and browsers: which programs count, how their window titles
//! are read, and when they want HDR. Feature doc: `docs/features/05-video.md`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::engine::TriggerId;
use crate::paths::NormPath;

/// The browser extension's page on the Chrome Web Store, for Chrome, Edge
/// and the other browsers built on Chromium.
pub const EXTENSION_STORE_URL: &str =
    "https://chromewebstore.google.com/detail/huedra/kilkdlcbhddpigcdffdnmdknjhlddlod";

/// The marker the browser extension puts in a tab's title while an HDR video
/// is open in it. Must match `MARKER` in `extras/browser/youtube.js`.
pub const MARKER: &str = "\u{27E6}HDR\u{27E7}";

/// Start of the marker the browser extension puts in a tab's title while a
/// video is full screen on a streaming site; the site key and `⟧` follow:
/// `⟦Video netflix⟧`. Must match `extras/browser/streaming.js`.
const SITE_MARKER_START: &str = "\u{27E6}Video ";

/// End of the extension's markers.
const MARKER_END: char = '\u{27E7}';

/// Prefix of the trigger ids of video, followed by the program key and the
/// process id: `video:mpv:1234`.
const TRIGGER_PREFIX: &str = "video:";

/// A program that plays video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoApp {
    /// VLC media player.
    Vlc,
    /// mpv.
    Mpv,
    /// mpv.net.
    MpvNet,
    /// MPC-HC.
    MpcHc,
    /// MPC-BE.
    MpcBe,
    /// PotPlayer.
    PotPlayer,
    /// Google Chrome.
    Chrome,
    /// Microsoft Edge.
    Edge,
    /// Mozilla Firefox.
    Firefox,
    /// Brave.
    Brave,
    /// Opera.
    Opera,
    /// Vivaldi.
    Vivaldi,
}

impl VideoApp {
    /// Every known program, players first.
    pub const ALL: [Self; 12] = [
        Self::Vlc,
        Self::Mpv,
        Self::MpvNet,
        Self::MpcHc,
        Self::MpcBe,
        Self::PotPlayer,
        Self::Chrome,
        Self::Edge,
        Self::Firefox,
        Self::Brave,
        Self::Opera,
        Self::Vivaldi,
    ];

    /// Recognizes a program by the file name of its path.
    pub fn from_exe(path: &str) -> Option<Self> {
        let path = NormPath::new(path);
        let app = match path.file_name() {
            "vlc.exe" => Self::Vlc,
            "mpv.exe" => Self::Mpv,
            "mpvnet.exe" => Self::MpvNet,
            "mpc-hc64.exe" | "mpc-hc.exe" => Self::MpcHc,
            "mpc-be64.exe" | "mpc-be.exe" => Self::MpcBe,
            "potplayer64.exe" | "potplayer.exe" | "potplayermini64.exe" | "potplayermini.exe" => {
                Self::PotPlayer
            }
            "chrome.exe" => Self::Chrome,
            "msedge.exe" => Self::Edge,
            "firefox.exe" => Self::Firefox,
            "brave.exe" => Self::Brave,
            "opera.exe" => Self::Opera,
            "vivaldi.exe" => Self::Vivaldi,
            _ => return None,
        };
        Some(app)
    }

    /// Key used in trigger ids; the same as in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Vlc => "vlc",
            Self::Mpv => "mpv",
            Self::MpvNet => "mpv-net",
            Self::MpcHc => "mpc-hc",
            Self::MpcBe => "mpc-be",
            Self::PotPlayer => "pot-player",
            Self::Chrome => "chrome",
            Self::Edge => "edge",
            Self::Firefox => "firefox",
            Self::Brave => "brave",
            Self::Opera => "opera",
            Self::Vivaldi => "vivaldi",
        }
    }

    /// Name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Vlc => "VLC",
            Self::Mpv => "mpv",
            Self::MpvNet => "mpv.net",
            Self::MpcHc => "MPC-HC",
            Self::MpcBe => "MPC-BE",
            Self::PotPlayer => "PotPlayer",
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
            Self::Firefox => "Firefox",
            Self::Brave => "Brave",
            Self::Opera => "Opera",
            Self::Vivaldi => "Vivaldi",
        }
    }

    /// Whether this is a web browser, which signals HDR video only through
    /// the extension's title marker.
    pub fn is_browser(self) -> bool {
        matches!(
            self,
            Self::Chrome | Self::Edge | Self::Firefox | Self::Brave | Self::Opera | Self::Vivaldi
        )
    }

    /// The choices offered for this program; While open applies to players
    /// only.
    pub fn choices(self) -> &'static [PlayerHdr] {
        if self.is_browser() {
            &[PlayerHdr::WhenHdrPlays, PlayerHdr::Never]
        } else {
            &[
                PlayerHdr::WhenHdrPlays,
                PlayerHdr::WhileOpen,
                PlayerHdr::Never,
            ]
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|app| app.key() == key)
    }
}

/// A streaming site that does not show whether a video is in HDR (its
/// streams are protected), where full-screen video can count as HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamingSite {
    /// Netflix.
    Netflix,
    /// Disney+.
    DisneyPlus,
    /// Prime Video.
    PrimeVideo,
    /// HBO Max.
    HboMax,
    /// Apple TV.
    AppleTv,
}

impl StreamingSite {
    /// Every known site.
    pub const ALL: [Self; 5] = [
        Self::Netflix,
        Self::DisneyPlus,
        Self::PrimeVideo,
        Self::HboMax,
        Self::AppleTv,
    ];

    /// Key used in the marker and the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Netflix => "netflix",
            Self::DisneyPlus => "disney-plus",
            Self::PrimeVideo => "prime-video",
            Self::HboMax => "hbo-max",
            Self::AppleTv => "apple-tv",
        }
    }

    /// Name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Netflix => "Netflix",
            Self::DisneyPlus => "Disney+",
            Self::PrimeVideo => "Prime Video",
            Self::HboMax => "HBO Max",
            Self::AppleTv => "Apple TV",
        }
    }

    /// The address the extension watches, as shown to the user.
    pub fn address(self) -> &'static str {
        match self {
            Self::Netflix => "netflix.com",
            Self::DisneyPlus => "disneyplus.com",
            Self::PrimeVideo => "primevideo.com",
            Self::HboMax => "hbomax.com",
            Self::AppleTv => "tv.apple.com",
        }
    }

    /// Recognizes a site by its key.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|site| site.key() == key)
    }

    /// The marker of this site: `⟦Video netflix⟧`.
    pub fn marker(self) -> String {
        format!("{SITE_MARKER_START}{}{MARKER_END}", self.key())
    }

    /// The site whose marker appears in a title.
    pub fn in_title(title: &str) -> Option<Self> {
        let start = title.find(SITE_MARKER_START)? + SITE_MARKER_START.len();
        let (key, _) = title.get(start..)?.split_once(MARKER_END)?;
        Self::from_key(key)
    }
}

/// What Huedra does for a video program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlayerHdr {
    /// Turn HDR on while it shows HDR video.
    #[default]
    WhenHdrPlays,
    /// Turn HDR on while it is open (players only).
    WhileOpen,
    /// Leave HDR as it is.
    Never,
}

impl PlayerHdr {
    /// Text shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::WhenHdrPlays => "When HDR video plays",
            Self::WhileOpen => "Whenever it's open",
            Self::Never => "Never",
        }
    }
}

/// One visible top-level window of a video program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoWindow {
    /// The window title.
    pub title: String,
    /// Whether the window covers its whole monitor without a title bar.
    pub full_screen: bool,
    /// The Windows number of the display it is on (2 for `\\.\DISPLAY2`),
    /// when known.
    pub monitor: Option<u32>,
}

/// What a video program's windows mean.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Assessment {
    /// A window shows HDR video but full screen is required and missing, so
    /// window moves must be watched.
    pub candidate: bool,
    /// HDR should be on for this program now.
    pub wants_hdr: bool,
    /// RTX Video HDR applies and a window is full screen without HDR video
    /// in its title: the program wants HDR if it is playing sound.
    pub sdr_full_screen: bool,
    /// Windows numbers of the displays showing that video: the windows
    /// that want HDR, or else the full-screen ones for SDR video.
    pub monitors: BTreeSet<u32>,
}

impl Assessment {
    /// What the program shows, given whether it is playing sound; `None`
    /// when HDR is not wanted.
    pub fn playing(&self, sound: bool) -> Option<VideoKind> {
        if self.wants_hdr {
            Some(VideoKind::Hdr)
        } else if self.sdr_full_screen && sound {
            Some(VideoKind::Sdr)
        } else {
            None
        }
    }
}

/// Why a video program wants HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoKind {
    /// HDR video, from the title (or While open).
    Hdr,
    /// SDR video in full screen with sound, which NVIDIA RTX Video HDR
    /// converts while Windows HDR is on.
    Sdr,
}

/// Whether a title names HDR video: one of the keywords appears as whole
/// words. Case, dots, dashes and underscores are ignored, so `Dolby Vision`
/// matches `Movie.Dolby.Vision.mkv`.
pub fn title_has_keyword(title: &str, keywords: &[String]) -> bool {
    let words = words(title);
    keywords.iter().any(|keyword| {
        let wanted = self::words(keyword);
        !wanted.is_empty()
            && words
                .windows(wanted.len())
                .any(|run| run == wanted.as_slice())
    })
}

/// The keywords typed as one line, separated by commas: trimmed, without
/// empty entries and without repeats (case ignored).
pub fn parse_keywords(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for keyword in text.split(',').map(str::trim).filter(|k| !k.is_empty()) {
        if !found.iter().any(|k| k.eq_ignore_ascii_case(keyword)) {
            found.push(keyword.to_owned());
        }
    }
    found
}

/// The keywords as one line, the form [`parse_keywords`] reads.
pub fn keywords_text(keywords: &[String]) -> String {
    keywords.join(", ")
}

/// Lower-case runs of letters and digits.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Decides for one program, given its mode, the keywords, the streaming
/// sites where full-screen video counts as HDR, and its windows. `rtx` is
/// true when SDR video counts too (RTX Video HDR is on and the setting
/// allows it). Video on a streaming site and SDR video must be full screen,
/// whatever `full_screen_only` says.
pub fn assess(
    app: VideoApp,
    mode: PlayerHdr,
    keywords: &[String],
    sites: &BTreeSet<StreamingSite>,
    full_screen_only: bool,
    rtx: bool,
    windows: &[VideoWindow],
) -> Assessment {
    match mode {
        PlayerHdr::Never => Assessment::default(),
        PlayerHdr::WhileOpen if !app.is_browser() => Assessment {
            wants_hdr: true,
            monitors: monitors(windows.iter()),
            ..Assessment::default()
        },
        PlayerHdr::WhileOpen | PlayerHdr::WhenHdrPlays => {
            let shows_hdr: Vec<&VideoWindow> = windows
                .iter()
                .filter(|w| {
                    if app.is_browser() {
                        w.title.contains(MARKER)
                    } else {
                        title_has_keyword(&w.title, keywords)
                    }
                })
                .collect();
            // The extension marks a streaming site's tab only in full
            // screen; the window must be full screen too.
            let on_site: Vec<&VideoWindow> = windows
                .iter()
                .filter(|w| {
                    app.is_browser()
                        && StreamingSite::in_title(&w.title).is_some_and(|s| sites.contains(&s))
                })
                .collect();
            let hdr_windows: Vec<&VideoWindow> = shows_hdr
                .iter()
                .copied()
                .filter(|w| !full_screen_only || w.full_screen)
                .chain(on_site.iter().copied().filter(|w| w.full_screen))
                .collect();
            let wants_hdr = !hdr_windows.is_empty();
            let sdr_full_screen = rtx && !wants_hdr && windows.iter().any(|w| w.full_screen);
            Assessment {
                candidate: !wants_hdr
                    && ((full_screen_only && !shows_hdr.is_empty()) || !on_site.is_empty()),
                wants_hdr,
                sdr_full_screen,
                monitors: if wants_hdr {
                    monitors(hdr_windows.into_iter())
                } else if sdr_full_screen {
                    monitors(windows.iter().filter(|w| w.full_screen))
                } else {
                    BTreeSet::new()
                },
            }
        }
    }
}

/// The displays the windows are on, where known.
fn monitors<'a>(windows: impl Iterator<Item = &'a VideoWindow>) -> BTreeSet<u32> {
    windows.filter_map(|w| w.monitor).collect()
}

/// The trigger id of HDR video in one process.
pub fn trigger_id(app: VideoApp, pid: u32) -> TriggerId {
    TriggerId::new(format!("{TRIGGER_PREFIX}{}:{pid}", app.key()))
}

/// The program behind a video trigger id, or `None` for other triggers.
pub fn app_of(id: &TriggerId) -> Option<VideoApp> {
    let rest = id.as_str().strip_prefix(TRIGGER_PREFIX)?;
    let (key, _pid) = rest.split_once(':')?;
    VideoApp::from_key(key)
}

/// The name used in reasons: "HDR video in mpv", or "Video in Chrome with
/// RTX Video HDR".
pub fn trigger_label(app: VideoApp, kind: VideoKind) -> String {
    match kind {
        VideoKind::Hdr => format!("HDR video in {}", app.label()),
        VideoKind::Sdr => format!("Video in {} with RTX Video HDR", app.label()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keywords() -> Vec<String> {
        ["HDR", "HDR10", "Dolby Vision", "DoVi", "BT.2020"]
            .map(String::from)
            .to_vec()
    }

    fn window(title: &str, full_screen: bool) -> VideoWindow {
        VideoWindow {
            title: title.into(),
            full_screen,
            monitor: None,
        }
    }

    #[test]
    fn recognizes_programs() {
        assert_eq!(
            VideoApp::from_exe(r"C:\Program Files\mpv\MPV.exe"),
            Some(VideoApp::Mpv),
            "case ignored"
        );
        assert_eq!(
            VideoApp::from_exe(r"C:\x\mpc-hc64.exe"),
            Some(VideoApp::MpcHc),
            "64-bit name"
        );
        assert_eq!(VideoApp::from_exe(r"C:\x\notepad.exe"), None, "other");
        assert!(VideoApp::Edge.is_browser(), "browser");
        assert!(!VideoApp::Vlc.is_browser(), "player");
        for app in VideoApp::ALL {
            assert_eq!(VideoApp::from_key(app.key()), Some(app), "key round trip");
        }
    }

    #[test]
    fn matches_keywords_as_words() {
        let k = keywords();
        assert!(
            title_has_keyword("Movie.2023.2160p.HDR.mkv - mpv", &k),
            "dots"
        );
        assert!(
            title_has_keyword("film_dolby_vision.mp4 - VLC", &k),
            "two words"
        );
        assert!(title_has_keyword("clip [hdr10].mkv", &k), "brackets");
        assert!(title_has_keyword("Demo BT.2020.mkv", &k), "dotted");
        assert!(
            !title_has_keyword("HDRezka.mkv - mpv", &k),
            "part of a word"
        );
        assert!(!title_has_keyword("SDR.mkv", &k), "no keyword");
        assert!(!title_has_keyword("x", &[String::new()]), "empty keyword");
    }

    #[test]
    fn keyword_lines() {
        assert_eq!(
            parse_keywords(" HDR, ,Dolby Vision,hdr ,HLG,"),
            ["HDR", "Dolby Vision", "HLG"].map(String::from).to_vec(),
            "trimmed, empty and repeated entries dropped"
        );
        assert!(parse_keywords(" , ").is_empty(), "nothing");
        let k = keywords();
        assert_eq!(parse_keywords(&keywords_text(&k)), k, "round trip");
    }

    #[test]
    fn choices_per_program() {
        assert_eq!(VideoApp::Mpv.choices().len(), 3, "player");
        assert!(
            !VideoApp::Firefox.choices().contains(&PlayerHdr::WhileOpen),
            "browsers have no While open"
        );
    }

    #[test]
    fn players_follow_titles_and_full_screen() {
        let k = keywords();
        let none = BTreeSet::new();
        let windowed = [window("a.HDR.mkv - mpv", false)];
        let full = [window("a.HDR.mkv - mpv", true)];
        let sdr = [window("a.mkv - mpv", true)];
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            false,
            &windowed,
        );
        assert_eq!(
            a,
            Assessment {
                candidate: true,
                ..Assessment::default()
            },
            "windowed waits"
        );
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            false,
            &full,
        );
        assert_eq!(
            a,
            Assessment {
                wants_hdr: true,
                ..Assessment::default()
            },
            "full screen"
        );
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            false,
            false,
            &windowed,
        );
        assert_eq!(
            a,
            Assessment {
                wants_hdr: true,
                ..Assessment::default()
            },
            "full screen not required"
        );
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            false,
            &sdr,
        );
        assert_eq!(a, Assessment::default(), "SDR file");
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhileOpen,
            &k,
            &none,
            true,
            false,
            &[],
        );
        assert!(a.wants_hdr, "while open");
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::Never,
            &k,
            &none,
            true,
            false,
            &full,
        );
        assert!(!a.wants_hdr, "never");
    }

    #[test]
    fn browsers_follow_the_marker_only() {
        let k = keywords();
        let none = BTreeSet::new();
        let marked = [window(
            &format!("{MARKER} Nature in HDR - YouTube - Google Chrome"),
            true,
        )];
        let plain = [window("Nature in HDR - YouTube - Google Chrome", true)];
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            false,
            &marked,
        );
        assert!(a.wants_hdr, "marker");
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            false,
            &plain,
        );
        assert!(!a.wants_hdr, "keywords do not count in a browser");
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhileOpen,
            &k,
            &none,
            true,
            false,
            &plain,
        );
        assert!(!a.wants_hdr, "while open does not apply to browsers");
    }

    #[test]
    fn sdr_video_with_rtx() {
        let k = keywords();
        let none = BTreeSet::new();
        let full = [window("Lecture - YouTube - Google Chrome", true)];
        let windowed = [window("Lecture - YouTube - Google Chrome", false)];
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            true,
            &full,
        );
        assert!(a.sdr_full_screen && !a.wants_hdr, "full screen SDR");
        assert_eq!(a.playing(true), Some(VideoKind::Sdr), "with sound");
        assert_eq!(a.playing(false), None, "silent");
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            false,
            true,
            &windowed,
        );
        assert_eq!(a.playing(true), None, "SDR video must be full screen");
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            false,
            &full,
        );
        assert_eq!(a.playing(true), None, "RTX Video HDR off");
        let a = assess(
            VideoApp::Vlc,
            PlayerHdr::Never,
            &k,
            &none,
            true,
            true,
            &full,
        );
        assert_eq!(a.playing(true), None, "never");
        let hdr = [window("a.HDR.mkv - mpv", true)];
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            true,
            &hdr,
        );
        assert_eq!(
            a.playing(false),
            Some(VideoKind::Hdr),
            "HDR title wins, sound or not"
        );
    }

    #[test]
    fn streaming_sites() {
        let k = keywords();
        let netflix = BTreeSet::from([StreamingSite::Netflix]);
        let title = format!(
            "{} Netflix - Google Chrome",
            StreamingSite::Netflix.marker()
        );
        assert_eq!(
            StreamingSite::in_title(&title),
            Some(StreamingSite::Netflix),
            "marker read"
        );
        assert_eq!(
            StreamingSite::in_title("\u{27E6}Video other\u{27E7} x"),
            None,
            "unknown site"
        );
        assert_eq!(StreamingSite::in_title("Netflix"), None, "no marker");
        for site in StreamingSite::ALL {
            assert_eq!(
                StreamingSite::from_key(site.key()),
                Some(site),
                "key round trip"
            );
        }
        let full = [window(&title, true)];
        let windowed = [window(&title, false)];
        let a = assess(
            VideoApp::Edge,
            PlayerHdr::WhenHdrPlays,
            &k,
            &netflix,
            false,
            false,
            &full,
        );
        assert_eq!(
            a.playing(false),
            Some(VideoKind::Hdr),
            "chosen site in full screen"
        );
        let a = assess(
            VideoApp::Edge,
            PlayerHdr::WhenHdrPlays,
            &k,
            &netflix,
            false,
            false,
            &windowed,
        );
        assert!(
            !a.wants_hdr && a.candidate,
            "windowed waits for full screen"
        );
        let a = assess(
            VideoApp::Edge,
            PlayerHdr::WhenHdrPlays,
            &k,
            &BTreeSet::from([StreamingSite::DisneyPlus]),
            true,
            false,
            &full,
        );
        assert_eq!(a, Assessment::default(), "site not chosen");
        let a = assess(
            VideoApp::Edge,
            PlayerHdr::Never,
            &k,
            &netflix,
            true,
            false,
            &full,
        );
        assert!(!a.wants_hdr, "never");
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhenHdrPlays,
            &k,
            &netflix,
            true,
            false,
            &full,
        );
        assert!(!a.wants_hdr, "players do not read site markers");
    }

    #[test]
    fn monitors_of_the_video() {
        let k = keywords();
        let none = BTreeSet::new();
        let on = |title: &str, full_screen, monitor| VideoWindow {
            title: title.into(),
            full_screen,
            monitor: Some(monitor),
        };
        let windows = [
            on("\u{27E6}HDR\u{27E7} Film - YouTube", true, 2),
            on("Mail", true, 1),
        ];
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            true,
            &windows,
        );
        assert_eq!(
            a.monitors,
            BTreeSet::from([2]),
            "only the HDR video's display"
        );
        let a = assess(
            VideoApp::Chrome,
            PlayerHdr::WhenHdrPlays,
            &k,
            &none,
            true,
            true,
            &windows[1..],
        );
        assert_eq!(a.monitors, BTreeSet::from([1]), "SDR video in full screen");
        let windowed = [on("a.HDR.mkv - mpv", false, 1), on("Playlist", false, 2)];
        let a = assess(
            VideoApp::Mpv,
            PlayerHdr::WhileOpen,
            &k,
            &none,
            true,
            false,
            &windowed,
        );
        assert_eq!(
            a.monitors,
            BTreeSet::from([1, 2]),
            "while open: every window"
        );
    }

    #[test]
    fn trigger_ids() {
        let id = trigger_id(VideoApp::MpvNet, 42);
        assert_eq!(id.as_str(), "video:mpv-net:42", "format");
        assert_eq!(app_of(&id), Some(VideoApp::MpvNet), "parsed");
        assert_eq!(app_of(&TriggerId::new("steam:1")), None, "game");
        assert_eq!(
            trigger_label(VideoApp::Mpv, VideoKind::Hdr),
            "HDR video in mpv",
            "label"
        );
        assert_eq!(
            trigger_label(VideoApp::Edge, VideoKind::Sdr),
            "Video in Edge with RTX Video HDR",
            "label of SDR video"
        );
    }
}
