//! Sample data for screenshots and tests, built only with the `sample`
//! feature (release builds leave it out). The window shows a fixed library
//! of popular games, three displays named after popular monitors and a
//! short history, and HDR is simulated: no display changes, no network
//! lookup is made and no tray process is needed. Settings are still read
//! from and saved to the copy's own settings file.
//! Feature doc: `docs/features/08-settings-window.md`.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use huedra_core::clock::{self, LocalTime};
use huedra_core::config::{Config, GameHdr, ManualDisplays};
use huedra_core::display::{Display, DisplayId, HdrState, HdrSupport, Nits};
use huedra_core::engine::Mode;
use huedra_core::gameinfo::{Cache, GameInfo, WikiHdr};
use huedra_core::ipc::{DisplayReport, HistoryEntry, HistoryKind, Request, StatusReport};
use huedra_core::library::{Game, GameId, Store};

/// Shown where the window would name the program's own path.
pub const EXE: &str = r"C:\Program Files\Huedra\huedra.exe";
/// Shown as the settings folder.
pub const DATA_FOLDER: &str = r"%LOCALAPPDATA%\Huedra";

/// The game that runs while the sample is in Automatic mode.
const RUNNING: (&str, Store) = ("1091500", Store::Steam);

/// Popular games with their HDR support as PCGamingWiki lists it.
const GAMES: [(&str, Store, &str, WikiHdr); 12] = [
    (
        "Cyberpunk 2077",
        Store::Steam,
        "1091500",
        WikiHdr::Supported,
    ),
    ("Elden Ring", Store::Steam, "1245620", WikiHdr::Supported),
    (
        "Forza Horizon 5",
        Store::Xbox,
        "forzahorizon5",
        WikiHdr::Supported,
    ),
    (
        "Hollow Knight",
        Store::Steam,
        "367520",
        WikiHdr::NotSupported,
    ),
    (
        "Stardew Valley",
        Store::Steam,
        "413150",
        WikiHdr::NotSupported,
    ),
    ("Terraria", Store::Steam, "105600", WikiHdr::NotSupported),
    (
        "Red Dead Redemption 2",
        Store::Rockstar,
        "rdr2",
        WikiHdr::Supported,
    ),
    (
        "Hogwarts Legacy",
        Store::Epic,
        "hogwartslegacy",
        WikiHdr::Supported,
    ),
    ("Celeste", Store::Steam, "504230", WikiHdr::NotSupported),
    ("Doom Eternal", Store::Steam, "782330", WikiHdr::Supported),
    ("Hades", Store::Steam, "1145360", WikiHdr::NotSupported),
    ("Minecraft", Store::Xbox, "minecraft", WikiHdr::Unknown),
];

/// Popular monitors: a 4K HDR display, an HDR ultrawide and one without HDR.
const DISPLAYS: [(&str, bool, f32); 3] = [
    ("ROG Strix XG27UCS", true, 1000.0),
    ("Alienware AW3423DWF", true, 1000.0),
    ("Dell P2422H", false, 0.0),
];

/// The sample library.
pub fn games() -> Vec<Game> {
    GAMES
        .iter()
        .map(|(name, store, id, _)| Game {
            id: GameId::new(*store, id),
            name: (*name).to_owned(),
            store: *store,
            install_dir: format!(r"D:\Games\{name}"),
            exe: None,
        })
        .collect()
}

/// PCGamingWiki's answers for the sample library, as if looked up.
pub fn info() -> Cache {
    let mut cache = Cache::default();
    for (name, store, id, hdr) in GAMES {
        cache.put(GameInfo {
            id: GameId::new(store, id),
            page: Some(name.replace(' ', "_")),
            hdr,
            hdr_notes: String::new(),
            ray_tracing: String::new(),
            fetched: u64::MAX / 2,
        });
    }
    cache
}

fn display_id(number: usize) -> DisplayId {
    DisplayId::new(format!(r"\\?\DISPLAY#SAMPLE{number}"))
}

/// The simulated state of the tray process.
struct Sim {
    mode: Mode,
    paused_until: Option<Instant>,
    /// Numbers of the displays in HDR.
    on: Vec<usize>,
    /// Set by the quick panel's display switches until the mode changes.
    forced: Option<Vec<usize>>,
    history: Vec<HistoryEntry>,
    config: PathBuf,
}

static SIM: Mutex<Option<Sim>> = Mutex::new(None);

/// The day and time `minutes` before `now`.
fn ago(now: LocalTime, minutes: u32) -> (String, String) {
    let mut day = now.date();
    let mut total = i64::from(now.hour) * 60 + i64::from(now.minute) - i64::from(minutes);
    while total < 0 {
        total += 24 * 60;
        day = clock::day_before(&day).unwrap_or_default();
    }
    (day, format!("{:02}:{:02}", total / 60, total % 60))
}

fn entry(
    now: LocalTime,
    minutes: u32,
    title: &str,
    detail: &str,
    kind: HistoryKind,
) -> HistoryEntry {
    let (day, time) = ago(now, minutes);
    HistoryEntry {
        time,
        day,
        title: title.to_owned(),
        detail: detail.to_owned(),
        kind,
    }
}

/// Starts the simulation: Automatic mode with Cyberpunk 2077 running, or
/// `HUEDRA_SAMPLE_MODE=sdr` / `hdr` to start in another mode. With
/// `HUEDRA_SAMPLE_FAILURE` set, the newest change is a failed switch. The
/// sample history ends a few minutes before `now`.
pub fn start(config: PathBuf, now: LocalTime) {
    let mode = match std::env::var("HUEDRA_SAMPLE_MODE").as_deref() {
        Ok("sdr") => Mode::AlwaysSdr,
        Ok("hdr") => Mode::AlwaysHdr,
        _ => Mode::Automatic,
    };
    let history = vec![
        entry(
            now,
            74,
            "HDR turned off",
            "HDR video in VLC ended",
            HistoryKind::Sdr,
        ),
        entry(
            now,
            88,
            "HDR turned on",
            "HDR video in VLC",
            HistoryKind::Hdr,
        ),
        entry(
            now,
            96,
            "Couldn't switch",
            "Windows didn't switch the display",
            HistoryKind::Failed,
        ),
        entry(
            now,
            1300,
            "HDR turned off",
            "Elden Ring closed",
            HistoryKind::Sdr,
        ),
        entry(
            now,
            1405,
            "HDR turned on",
            "Elden Ring started",
            HistoryKind::Hdr,
        ),
        entry(
            now,
            1600,
            "Changed outside Huedra",
            "HDR was turned off in Windows Settings",
            HistoryKind::External,
        ),
        entry(
            now,
            1613,
            "HDR turned on",
            "Forza Horizon 5 started",
            HistoryKind::Hdr,
        ),
    ];
    let mut sim = Sim {
        mode,
        paused_until: None,
        on: Vec::new(),
        forced: None,
        history,
        config,
    };
    sim.on = sim.wanted(&sim.load());
    if mode == Mode::Automatic && !sim.on.is_empty() {
        sim.history.insert(
            0,
            entry(
                now,
                2,
                "HDR turned on",
                "Cyberpunk 2077 started",
                HistoryKind::Hdr,
            ),
        );
    }
    if std::env::var_os("HUEDRA_SAMPLE_FAILURE").is_some() {
        sim.history.insert(
            0,
            entry(
                now,
                1,
                "Couldn't switch",
                "Windows didn't switch the display",
                HistoryKind::Failed,
            ),
        );
    }
    *SIM.lock().unwrap_or_else(PoisonError::into_inner) = Some(sim);
}

impl Sim {
    fn load(&self) -> Config {
        Config::load(&self.config).unwrap_or_default()
    }

    fn paused(&self) -> Option<u32> {
        let left = self.paused_until?.checked_duration_since(Instant::now())?;
        Some(u32::try_from(left.as_secs().div_ceil(60)).unwrap_or(u32::MAX))
    }

    /// The displays the mode and settings turn to HDR.
    fn wanted(&self, config: &Config) -> Vec<usize> {
        let controlled: Vec<usize> = DISPLAYS
            .iter()
            .enumerate()
            .filter(|(i, d)| d.1 && config.is_managed(display_id(i + 1).as_str()))
            .map(|(i, _)| i + 1)
            .collect();
        match self.mode {
            Mode::AlwaysSdr => Vec::new(),
            Mode::AlwaysHdr if config.displays.manual == ManualDisplays::Main => {
                controlled.into_iter().filter(|n| *n == 1).collect()
            }
            Mode::AlwaysHdr => controlled,
            Mode::Automatic => {
                let id = GameId::new(RUNNING.1, RUNNING.0);
                if config.game(&id).map(|g| g.hdr).unwrap_or_default()
                    != GameHdr::EnableWhileRunning
                {
                    Vec::new()
                } else if config.displays.content_only {
                    controlled.into_iter().filter(|n| *n == 1).collect()
                } else {
                    controlled
                }
            }
        }
    }

    fn reason(&self) -> String {
        if let Some(minutes) = self.paused() {
            return format!("Paused for {minutes} more minutes");
        }
        match self.mode {
            Mode::Automatic if !self.on.is_empty() => "Cyberpunk 2077 is running".to_owned(),
            Mode::Automatic => "Nothing needs HDR right now".to_owned(),
            mode => format!("{} mode", mode.label()),
        }
    }

    /// Follows the settings and the mode, as the tray would, and records
    /// each change.
    fn step(&mut self, config: &Config) {
        if self.paused_until.is_some() && self.paused().is_none() {
            self.paused_until = None;
        }
        if self.paused().is_some() {
            return;
        }
        let wanted = self.forced.clone().unwrap_or_else(|| self.wanted(config));
        if wanted == self.on {
            return;
        }
        let now = huedra_win::clock::now();
        let (title, kind) = if wanted.is_empty() {
            ("HDR turned off", HistoryKind::Sdr)
        } else {
            ("HDR turned on", HistoryKind::Hdr)
        };
        let detail = match self.mode {
            Mode::Automatic if wanted.is_empty() => "Cyberpunk 2077 closed".to_owned(),
            Mode::Automatic => "Cyberpunk 2077 started".to_owned(),
            mode => format!("{} mode", mode.label()),
        };
        self.history.insert(
            0,
            HistoryEntry {
                time: now.hour_minute(),
                day: now.date(),
                title: title.to_owned(),
                detail,
                kind,
            },
        );
        self.on = wanted;
    }
}

/// Installed color profiles, as a PC with a calibrated monitor has them.
pub fn profiles() -> huedra_core::color::Installed {
    let names = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
    huedra_core::color::Installed {
        sdr: names(&[
            "Display Calibration 2026-10.icm",
            "HDR Calibration 2026-10.icc",
            "sRGB Color Space Profile.icm",
        ]),
        hdr: names(&["HDR Calibration 2026-10.icc"]),
    }
}

/// The displays as Windows would report them, with the simulated HDR state.
pub fn displays() -> Vec<Display> {
    let on = SIM
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|s| s.on.clone())
        .unwrap_or_default();
    DISPLAYS
        .iter()
        .enumerate()
        .map(|(i, (name, hdr, peak))| Display {
            id: display_id(i + 1),
            number: u32::try_from(i + 1).unwrap_or(0),
            name: (*name).to_owned(),
            primary: i == 0,
            support: if *hdr {
                HdrSupport::Supported
            } else {
                HdrSupport::Unsupported
            },
            state: if on.contains(&(i + 1)) {
                HdrState::On
            } else {
                HdrState::Off
            },
            peak: Nits::new(*peak),
            // As Windows reports it while HDR is on.
            sdr_brightness: on
                .contains(&(i + 1))
                .then(|| huedra_core::display::SdrBrightness::from_level(3000)),
        })
        .collect()
}

/// The simulated tray's answer to Status.
pub fn status() -> Option<StatusReport> {
    let mut guard = SIM.lock().unwrap_or_else(PoisonError::into_inner);
    let sim = guard.as_mut()?;
    let config = sim.load();
    sim.step(&config);
    let hdr_on = !sim.on.is_empty();
    let reason = sim.reason();
    Some(StatusReport {
        mode: sim.mode,
        paused_minutes: sim.paused(),
        hdr_on,
        summary: format!("{} · {reason}", if hdr_on { "HDR on" } else { "HDR off" }),
        active_games: if sim.mode == Mode::Automatic && hdr_on {
            vec![GameId::new(RUNNING.1, RUNNING.0).as_str().to_owned()]
        } else {
            Vec::new()
        },
        reason,
        busy: false,
        displays: DISPLAYS
            .iter()
            .enumerate()
            .filter(|(i, d)| d.1 && config.is_managed(display_id(i + 1).as_str()))
            .map(|(i, (name, _, _))| DisplayReport {
                id: display_id(i + 1).as_str().to_owned(),
                label: format!("Display {} ({name})", i + 1),
                hdr_on: sim.on.contains(&(i + 1)),
                detail: String::new(),
            })
            .collect(),
        history: sim.history.clone(),
        shortcut_note: String::new(),
        never: if sim.mode == Mode::Automatic && hdr_on && sim.forced.is_none() {
            "Cyberpunk 2077".to_owned()
        } else {
            String::new()
        },
    })
}

/// Carries out a request the window sends to the tray.
pub fn handle(request: &Request) {
    let mut guard = SIM.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(sim) = guard.as_mut() else {
        return;
    };
    match request {
        Request::SetMode { mode } => {
            sim.mode = *mode;
            sim.forced = None;
        }
        Request::SetDisplayHdr { display, on } => {
            let Some(number) = (1..=DISPLAYS.len()).find(|n| display_id(*n).as_str() == display)
            else {
                return;
            };
            let mut set = sim.on.clone();
            set.retain(|n| *n != number);
            if *on {
                set.push(number);
                set.sort_unstable();
            }
            match sim.mode {
                Mode::AlwaysSdr if *on => sim.mode = Mode::Automatic,
                Mode::AlwaysHdr if !*on => sim.mode = Mode::Automatic,
                _ => {}
            }
            sim.forced = Some(set);
        }
        Request::NeverForActive => {
            let mut config = sim.load();
            config.game_mut(&GameId::new(RUNNING.1, RUNNING.0)).hdr = GameHdr::DoNotChange;
            let _ = config.save(&sim.config);
        }
        Request::Pause { minutes } => {
            sim.paused_until = Some(Instant::now() + Duration::from_secs(u64::from(*minutes) * 60));
        }
        Request::Resume => sim.paused_until = None,
        _ => {}
    }
}
