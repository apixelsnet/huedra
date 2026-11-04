//! The settings file `config.toml`: schema, defaults, limits and version
//! upgrades. Feature doc: `docs/features/10-config.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::{Mode, Target, Trigger};
use crate::files;
use crate::library::{Game, GameId, Store};
use crate::paths::NormPath;
use crate::video::{self, Assessment, PlayerHdr, StreamingSite, VideoApp, VideoKind, VideoWindow};

/// Schema version written by this build.
pub const VERSION: u32 = 1;
/// Files larger than this are refused before parsing.
pub const MAX_BYTES: u64 = 1 << 20;
/// Upper limit for start delays.
pub const MAX_DELAY: Seconds = Seconds(120);
/// Upper limit for hold times.
pub const MAX_HOLD: Seconds = Seconds(600);
/// Upper limit for retry attempts.
pub const MAX_RETRIES: u32 = 5;

/// A whole number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seconds(u32);

impl Seconds {
    /// Wraps a number of seconds.
    pub const fn new(secs: u32) -> Self {
        Self(secs)
    }

    /// The number of seconds.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// As a [`Duration`].
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

/// How updates are installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdatePolicy {
    /// Install signed releases automatically.
    Install,
    /// Show what is new and install when the user chooses to.
    #[default]
    Ask,
    /// Do not check.
    Off,
}

/// What closing the settings window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CloseAction {
    /// Close the window; Huedra keeps running in the notification area.
    #[default]
    KeepRunning,
    /// Close the window and quit Huedra.
    Quit,
}

/// What Huedra does while a game runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameHdr {
    /// Turn HDR on while the game runs.
    #[default]
    EnableWhileRunning,
    /// Leave HDR as it is.
    DoNotChange,
    /// The game switches HDR itself; leave it as it is.
    GameControls,
}

impl GameHdr {
    /// Text shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::EnableWhileRunning => "Enable while running",
            Self::DoNotChange => "Do not change",
            Self::GameControls => "Game controls HDR",
        }
    }
}

/// The whole settings file. Plain data, so its fields are public.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Schema version.
    pub version: u32,
    /// Mode at start.
    pub mode: Mode,
    /// General settings.
    pub general: General,
    /// Display settings.
    pub displays: DisplaySettings,
    /// Video players and browsers.
    pub video: VideoSettings,
    /// Per-game settings and games added by hand.
    #[serde(rename = "game", skip_serializing_if = "Vec::is_empty")]
    pub games: Vec<GameSettings>,
}

/// The General page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Start in the tray when Windows starts.
    pub start_with_windows: bool,
    /// Update policy.
    pub updates: UpdatePolicy,
    /// What closing the settings window does.
    pub on_close: CloseAction,
    /// Look up games on PCGamingWiki.
    pub game_info: bool,
    /// Default start delay.
    #[serde(rename = "start_delay_secs")]
    pub start_delay: Seconds,
    /// Default hold time.
    #[serde(rename = "hold_secs")]
    pub hold: Seconds,
    /// Extra attempts when a display refuses a change.
    pub retry_attempts: u32,
    /// Write window titles to the log.
    pub log_window_titles: bool,
    /// Register the keyboard shortcuts.
    pub hotkeys: bool,
    /// The disclaimer shown on the first start was accepted. Until then the
    /// tray does not start and HDR is never changed.
    pub disclaimer_accepted: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            start_with_windows: true,
            updates: UpdatePolicy::Ask,
            on_close: CloseAction::KeepRunning,
            game_info: true,
            start_delay: Seconds(0),
            hold: Seconds(10),
            retry_attempts: 2,
            log_window_titles: false,
            hotkeys: true,
            disclaimer_accepted: false,
        }
    }
}

/// The Displays page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    /// Device paths of HDR displays the user excluded. Every other HDR
    /// display is managed, including ones connected later.
    pub unmanaged: Vec<String>,
    /// A game or video turns HDR on only for the display its window is
    /// on. Off: every managed display switches together.
    pub content_only: bool,
    /// The displays Always HDR mode and the shortcut turn on.
    pub manual: ManualDisplays,
}

/// The displays Always HDR mode and the "Turn HDR on or off" shortcut turn
/// on, which have no window to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManualDisplays {
    /// The main display (the Windows primary display), so a second
    /// monitor stays in SDR.
    #[default]
    Main,
    /// Every managed display.
    All,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            unmanaged: Vec::new(),
            content_only: true,
            manual: ManualDisplays::Main,
        }
    }
}

/// The Video players page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// Words in a player's title that mark HDR video.
    pub keywords: Vec<String>,
    /// Video in a window does not change the display.
    pub full_screen_only: bool,
    /// With NVIDIA RTX Video HDR on, full-screen SDR video with sound turns
    /// HDR on too, so the driver can convert it.
    pub sdr_with_rtx: bool,
    /// Streaming sites where full-screen video counts as HDR (the browser
    /// extension marks it); none unless chosen.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub sites: BTreeSet<StreamingSite>,
    /// Overrides the default hold time for video.
    #[serde(rename = "hold_secs", skip_serializing_if = "Option::is_none")]
    pub hold: Option<Seconds>,
    /// Per-program choices; programs not listed use
    /// [`PlayerHdr::WhenHdrPlays`].
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub apps: BTreeMap<VideoApp, PlayerHdr>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            keywords: default_keywords(),
            full_screen_only: true,
            sdr_with_rtx: true,
            sites: BTreeSet::new(),
            hold: None,
            apps: BTreeMap::new(),
        }
    }
}

/// The words that mark HDR video in a player's title unless changed.
pub fn default_keywords() -> Vec<String> {
    ["HDR", "HDR10", "Dolby Vision", "DoVi", "BT.2020"]
        .map(String::from)
        .to_vec()
}

impl VideoSettings {
    /// The choice for one program.
    pub fn mode(&self, app: VideoApp) -> PlayerHdr {
        self.apps.get(&app).copied().unwrap_or_default()
    }

    /// Sets the choice for one program; the default is not written.
    pub fn set_mode(&mut self, app: VideoApp, mode: PlayerHdr) {
        if mode == PlayerHdr::default() {
            self.apps.remove(&app);
        } else {
            self.apps.insert(app, mode);
        }
    }

    /// Sets the keywords from one line separated by commas; an empty line
    /// restores the default words.
    pub fn set_keywords_text(&mut self, text: &str) {
        let keywords = video::parse_keywords(text);
        self.keywords = if keywords.is_empty() {
            default_keywords()
        } else {
            keywords
        };
    }

    /// Turns the rule for one streaming site on or off.
    pub fn set_site(&mut self, site: StreamingSite, on: bool) {
        if on {
            self.sites.insert(site);
        } else {
            self.sites.remove(&site);
        }
    }

    /// Decides for one program from its windows; `rtx_on` is whether
    /// NVIDIA RTX Video HDR is turned on.
    pub fn assess(&self, app: VideoApp, rtx_on: bool, windows: &[VideoWindow]) -> Assessment {
        video::assess(
            app,
            self.mode(app),
            &self.keywords,
            &self.sites,
            self.full_screen_only,
            self.sdr_with_rtx && rtx_on,
            windows,
        )
    }
}

/// Settings for one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSettings {
    /// The game.
    pub id: GameId,
    /// Name, for games added by hand.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Program, for games added by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    /// What to do while it runs.
    #[serde(default)]
    pub hdr: GameHdr,
    /// Overrides the default start delay.
    #[serde(
        default,
        rename = "start_delay_secs",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_delay: Option<Seconds>,
    /// Overrides the default hold time.
    #[serde(default, rename = "hold_secs", skip_serializing_if = "Option::is_none")]
    pub hold: Option<Seconds>,
    /// Start through `huedra launch` (shown in the Games page).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub launch_with_hdr: bool,
    /// Turn HDR on as soon as Steam reports the game running, before its
    /// window shows (Steam games only; see `steam_start`). Unset means on;
    /// only a game switched off writes `false`. See [`Config::steam_start`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_start: Option<bool>,
}

impl GameSettings {
    /// Settings with every value at its default.
    pub fn new(id: GameId) -> Self {
        Self {
            id,
            name: String::new(),
            exe: None,
            hdr: GameHdr::default(),
            start_delay: None,
            hold: None,
            launch_with_hdr: false,
            steam_start: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            mode: Mode::Automatic,
            general: General::default(),
            displays: DisplaySettings::default(),
            video: VideoSettings::default(),
            games: Vec::new(),
        }
    }
}

/// The settings file could not be used.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Reading or writing failed.
    #[error("could not access the settings file: {0}")]
    Io(#[from] std::io::Error),
    /// The text is not valid TOML or does not fit the schema.
    #[error("the settings file is not valid: {0}")]
    Invalid(String),
    /// Written by a newer version of Huedra.
    #[error("the settings file was written by a newer version of Huedra (schema {0})")]
    Newer(u32),
}

impl Config {
    /// Parses settings text, upgrading older schemas and bringing values into
    /// range.
    ///
    /// # Errors
    /// [`ConfigError`] for oversized, invalid or newer files.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        // Notepad and Windows PowerShell 5.1 may start the file with a BOM.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        if text.len() as u64 > MAX_BYTES {
            return Err(ConfigError::Invalid("larger than 1 MB".into()));
        }
        let table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| ConfigError::Invalid(e.message().to_owned()))?;
        let version = table
            .get("version")
            .and_then(toml::Value::as_integer)
            .unwrap_or(i64::from(VERSION));
        if version > i64::from(VERSION) {
            return Err(ConfigError::Newer(
                u32::try_from(version).unwrap_or(u32::MAX),
            ));
        }
        // Version 1 is the first schema; upgrades from later versions go here.
        let mut config: Self = table
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::Invalid(e.message().to_owned()))?;
        config.version = VERSION;
        config.clamp();
        Ok(config)
    }

    /// The settings as TOML text.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] if serialization fails (it should not).
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(|e| ConfigError::Invalid(e.to_string()))
    }

    /// Loads the file; a missing file gives the defaults.
    ///
    /// # Errors
    /// See [`Self::parse`].
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match files::read_limited(path, MAX_BYTES)? {
            Some(text) => Self::parse(&text),
            None => Ok(Self::default()),
        }
    }

    /// Saves the file atomically.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        files::write_atomic(path, self.to_toml()?.as_bytes())?;
        Ok(())
    }

    fn clamp(&mut self) {
        let g = &mut self.general;
        g.start_delay = g.start_delay.min(MAX_DELAY);
        g.hold = g.hold.min(MAX_HOLD);
        g.retry_attempts = g.retry_attempts.min(MAX_RETRIES);
        self.video.hold = self.video.hold.map(|h| h.min(MAX_HOLD));
        for game in &mut self.games {
            game.start_delay = game.start_delay.map(|d| d.min(MAX_DELAY));
            game.hold = game.hold.map(|h| h.min(MAX_HOLD));
        }
    }

    /// Settings for a game, if any were saved.
    pub fn game(&self, id: &GameId) -> Option<&GameSettings> {
        self.games.iter().find(|g| &g.id == id)
    }

    /// Settings for a game, created with defaults when missing.
    pub fn game_mut(&mut self, id: &GameId) -> &mut GameSettings {
        if let Some(i) = self.games.iter().position(|g| &g.id == id) {
            &mut self.games[i]
        } else {
            self.games.push(GameSettings::new(id.clone()));
            let last = self.games.len() - 1;
            &mut self.games[last]
        }
    }

    /// Games the user added by hand, as library entries.
    pub fn manual_games(&self) -> Vec<Game> {
        self.games
            .iter()
            .filter_map(|s| {
                let exe = s.exe.clone()?;
                let folder = NormPath::new(&exe)
                    .parent()
                    .map(|_| parent_as_written(&exe))
                    .unwrap_or_default();
                let name = if s.name.is_empty() {
                    crate::paths::display_name(&exe).to_owned()
                } else {
                    s.name.clone()
                };
                Some(Game {
                    id: s.id.clone(),
                    name,
                    store: Store::Manual,
                    install_dir: folder,
                    exe: Some(exe),
                })
            })
            .collect()
    }

    /// The trigger a running game produces, or `None` when Huedra should
    /// leave HDR alone for it.
    pub fn trigger_for(&self, game: &Game) -> Option<Trigger> {
        let settings = self.game(&game.id);
        let hdr = settings.map(|s| s.hdr).unwrap_or_default();
        if hdr != GameHdr::EnableWhileRunning {
            return None;
        }
        Some(Trigger {
            label: game.name.clone(),
            delay: settings
                .and_then(|s| s.start_delay)
                .unwrap_or(self.general.start_delay)
                .duration(),
            hold: settings
                .and_then(|s| s.hold)
                .unwrap_or(self.general.hold)
                .duration(),
            target: Target::All,
        })
    }

    /// Whether "Turn HDR on when Steam starts the game" is on for a game:
    /// on unless the game was switched off.
    pub fn steam_start(&self, id: &GameId) -> bool {
        self.game(id).and_then(|s| s.steam_start).unwrap_or(true)
    }

    /// The trigger that turns HDR on when Steam starts a game, for a Steam
    /// game set to Enable while running with "Turn HDR on when Steam starts
    /// it" on: no delay (its window is not there yet), the game's hold time.
    pub fn steam_start_trigger(&self, game: &Game) -> Option<Trigger> {
        if game.store != Store::Steam || !self.steam_start(&game.id) {
            return None;
        }
        self.trigger_for(game).map(|trigger| Trigger {
            delay: Duration::ZERO,
            ..trigger
        })
    }

    /// The trigger of video in a program.
    pub fn video_trigger(&self, app: VideoApp, kind: VideoKind) -> Trigger {
        Trigger {
            label: video::trigger_label(app, kind),
            delay: self.general.start_delay.duration(),
            hold: self.video.hold.unwrap_or(self.general.hold).duration(),
            target: Target::All,
        }
    }

    /// Whether an HDR-capable display is managed.
    pub fn is_managed(&self, device_path: &str) -> bool {
        !self
            .displays
            .unmanaged
            .iter()
            .any(|d| d.eq_ignore_ascii_case(device_path))
    }

    /// Number of attempts per switch (first try plus retries).
    pub fn attempts(&self) -> u32 {
        1 + self.general.retry_attempts
    }
}

/// The folder part of a path, keeping its original case.
fn parent_as_written(path: &str) -> String {
    path.rfind(['\\', '/'])
        .map(|i| path[..i].to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let config = Config::default();
        let text = config.to_toml().expect("serialize");
        assert_eq!(Config::parse(&text).expect("parse"), config, "round trip");
        assert_eq!(Config::parse("").expect("empty"), config, "empty file");
        assert_eq!(config.general.hold, Seconds(10), "default hold");
        assert!(config.general.game_info, "game info on by default");
        assert_eq!(config.general.updates, UpdatePolicy::Ask, "updates");
        assert_eq!(
            config.general.on_close,
            CloseAction::KeepRunning,
            "closing the window keeps Huedra running"
        );
        assert_eq!(config.attempts(), 3, "one try plus two retries");
    }

    #[test]
    fn close_action() {
        let config = Config::parse("[general]\non_close = \"quit\"\n").expect("parse");
        assert_eq!(config.general.on_close, CloseAction::Quit, "quit");
        let text = config.to_toml().expect("serialize");
        assert!(text.contains("on_close = \"quit\""), "written as {text}");
    }

    #[test]
    fn reads_and_clamps() {
        let text = r#"
            version = 1
            mode = "always-hdr"
            unknown_key = "ignored"
            [general]
            hold_secs = 99999
            [[game]]
            id = "steam:1091500"
            hdr = "do-not-change"
            [[game]]
            id = "manual:tool"
            name = "Tool"
            exe = 'C:\Games\Tool\Tool.exe'
            hold_secs = 5
        "#;
        let config = Config::parse(text).expect("parse");
        assert_eq!(config.mode, Mode::AlwaysHdr, "mode");
        assert_eq!(config.general.hold, MAX_HOLD, "clamped");
        let manual = config.manual_games();
        assert_eq!(manual.len(), 1, "one manual game");
        assert_eq!(manual[0].install_dir, r"C:\Games\Tool", "folder");
        let trigger = config.trigger_for(&manual[0]).expect("enabled by default");
        assert_eq!(trigger.hold, Duration::from_secs(5), "own hold time");
        let steam = Game {
            id: GameId::from_raw("steam:1091500"),
            name: "Cyberpunk 2077".into(),
            store: Store::Steam,
            install_dir: r"D:\x".into(),
            exe: None,
        };
        assert!(config.trigger_for(&steam).is_none(), "do not change");
    }

    #[test]
    fn video_settings() {
        let text = r#"
            [video]
            full_screen_only = false
            hold_secs = 4
            [video.apps]
            mpv-net = "while-open"
            chrome = "never"
        "#;
        let config = Config::parse(text).expect("parse");
        assert_eq!(
            config.video.mode(VideoApp::MpvNet),
            PlayerHdr::WhileOpen,
            "listed"
        );
        assert_eq!(
            config.video.mode(VideoApp::Chrome),
            PlayerHdr::Never,
            "browser"
        );
        assert_eq!(
            config.video.mode(VideoApp::Vlc),
            PlayerHdr::WhenHdrPlays,
            "default"
        );
        assert_eq!(config.video.keywords.len(), 5, "default keywords kept");
        let trigger = config.video_trigger(VideoApp::Mpv, VideoKind::Hdr);
        assert_eq!(trigger.label, "HDR video in mpv", "label");
        assert_eq!(trigger.hold, Duration::from_secs(4), "video hold");
        let text = config.to_toml().expect("serialize");
        assert!(
            text.contains("mpv-net = \"while-open\""),
            "keys written: {text}"
        );
        assert_eq!(Config::parse(&text).expect("again"), config, "round trip");
        assert!(
            Config::default().video.full_screen_only,
            "full screen by default"
        );
        assert!(config.video.sdr_with_rtx, "SDR video with RTX by default");
        let off = Config::parse("[video]\nsdr_with_rtx = false").expect("parse");
        assert!(!off.video.sdr_with_rtx, "turned off");
        assert!(config.video.sites.is_empty(), "no sites by default");
        let mut sites =
            Config::parse("[video]\nsites = [\"hbo-max\", \"netflix\"]").expect("parse");
        assert_eq!(
            sites.video.sites,
            BTreeSet::from([StreamingSite::Netflix, StreamingSite::HboMax]),
            "sites read"
        );
        sites.video.set_site(StreamingSite::HboMax, false);
        let text = sites.to_toml().expect("serialize");
        assert!(text.contains("sites = [\"netflix\"]"), "written: {text}");
        assert_eq!(Config::parse(&text).expect("again"), sites, "round trip");
    }

    #[test]
    fn video_edits() {
        let mut video = VideoSettings::default();
        video.set_mode(VideoApp::Vlc, PlayerHdr::Never);
        assert_eq!(video.mode(VideoApp::Vlc), PlayerHdr::Never, "set");
        video.set_mode(VideoApp::Vlc, PlayerHdr::WhenHdrPlays);
        assert!(video.apps.is_empty(), "default not written");
        video.set_keywords_text("HLG, PQ");
        assert_eq!(video.keywords, ["HLG", "PQ"].map(String::from), "typed");
        video.set_keywords_text("  ");
        assert_eq!(video.keywords, default_keywords(), "empty restores");
    }

    #[test]
    fn accepts_byte_order_mark() {
        let config = Config::parse("\u{feff}mode = \"always-sdr\"").expect("parse");
        assert_eq!(config.mode, Mode::AlwaysSdr, "BOM ignored");
    }

    #[test]
    fn rejects_bad_files() {
        assert!(
            matches!(Config::parse("version = 99"), Err(ConfigError::Newer(99))),
            "newer"
        );
        assert!(
            matches!(Config::parse("mode = 3"), Err(ConfigError::Invalid(_))),
            "wrong type"
        );
        let huge = "#".repeat(2 << 20);
        assert!(
            matches!(Config::parse(&huge), Err(ConfigError::Invalid(_))),
            "size"
        );
    }

    #[test]
    fn game_mut_inserts_once() {
        let mut config = Config::default();
        let id = GameId::from_raw("steam:1");
        config.game_mut(&id).hdr = GameHdr::DoNotChange;
        config.game_mut(&id).hold = Some(Seconds(3));
        assert_eq!(config.games.len(), 1, "one entry");
        assert!(config.is_managed("x"), "managed by default");
        config.displays.unmanaged.push("X".into());
        assert!(!config.is_managed("x"), "excluded");
    }

    #[test]
    fn steam_start_trigger() {
        let game = |store: Store, id: &str| Game {
            id: GameId::new(store, id),
            name: "Battlefield 6".into(),
            store,
            install_dir: r"D:\Steam\common\Battlefield 6".into(),
            exe: None,
        };
        let steam = game(Store::Steam, "2807960");
        let mut config = Config::default();
        assert!(
            config.steam_start_trigger(&steam).is_some(),
            "on by default, with no entry for the game"
        );
        let settings = config.game_mut(&steam.id);
        settings.start_delay = Some(Seconds(8));
        settings.hold = Some(Seconds(4));
        let trigger = config.steam_start_trigger(&steam);
        assert_eq!(
            trigger.as_ref().map(|t| (t.delay, t.hold)),
            Some((Duration::ZERO, Duration::from_secs(4))),
            "no delay, the game's hold"
        );
        let text = toml::to_string(&config).unwrap_or_default();
        assert!(
            text.contains("steam:2807960") && !text.contains("steam_start"),
            "the default is not written"
        );
        config.game_mut(&steam.id).steam_start = Some(false);
        assert!(config.steam_start_trigger(&steam).is_none(), "switched off");
        let text = toml::to_string(&config).unwrap_or_default();
        assert!(text.contains("steam_start = false"), "off is written");
        config.game_mut(&steam.id).steam_start = None;
        config.game_mut(&steam.id).hdr = GameHdr::GameControls;
        assert!(
            config.steam_start_trigger(&steam).is_none(),
            "only for Enable while running"
        );
        let epic = game(Store::Epic, "Fn");
        assert!(config.steam_start_trigger(&epic).is_none(), "Steam only");
        let old = Config::parse("[[game]]\nid = \"steam:1\"\nsteam_start = true\n").ok();
        assert!(
            old.is_some_and(|c| c.steam_start(&GameId::from_raw("steam:1"))),
            "1.1.1 files still read"
        );
    }

    #[test]
    fn content_only_by_default() {
        assert!(Config::default().displays.content_only, "default on");
        let config = Config::parse("[displays]\nunmanaged = []\n").ok();
        assert!(
            config.is_some_and(|c| c.displays.content_only),
            "missing key keeps the default"
        );
        let config = Config::parse("[displays]\ncontent_only = false\n").ok();
        assert!(
            config.is_some_and(|c| !c.displays.content_only),
            "turned off"
        );
    }

    #[test]
    fn manual_displays_default_to_the_main_one() {
        assert_eq!(
            Config::default().displays.manual,
            ManualDisplays::Main,
            "default"
        );
        let config = Config::parse("[displays]\nmanual = \"all\"\n").ok();
        assert_eq!(
            config.map(|c| c.displays.manual),
            Some(ManualDisplays::All),
            "every display"
        );
    }
}
