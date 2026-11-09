//! The tray role: watches programs and displays, runs the engine, shows the
//! icon and menu, and answers the command line and the settings window.
//! Feature docs: `02-engine.md`, `03-watchers.md`, `05-video.md`,
//! `06-launch-with-hdr.md`, `07-tray-hotkeys.md`.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Context as _;
use huedra_core::color::{self, ColorState, GameColor};
use huedra_core::command::{TrayStart, startup_command, startup_program};
use huedra_core::config::{Config, GameHdr, ManualDisplays, UpdatePolicy};
use huedra_core::display::{Display, DisplayId, HdrState};
use huedra_core::engine::{
    Action, Engine, Mode, Note, NoteKind, Outcome, Phase, Status, Target, Trigger, TriggerId,
};
use huedra_core::files;
use huedra_core::icon::TrayIcon;
use huedra_core::ipc::{DisplayReport, HistoryEntry, HistoryKind, Request, Response, StatusReport};
use huedra_core::launch;
use huedra_core::library::{GameId, Library};
use huedra_core::logfile::{self, LogFile};
use huedra_core::steam_start::{self, SteamStarts};
use huedra_core::tracker::{Exit, Pid, Tracker};
use huedra_core::update::{self, UpdateState, Version};
use huedra_core::video::{self, PlayerHdr, VideoApp, VideoKind};
use huedra_win::event_loop::{Event, EventLoop, Flow, TimerId, Waker};
use huedra_win::hotkey::{Hotkey, Hotkeys};
use huedra_win::menu::{self, MenuEntry};
use huedra_win::pipe::{PipeError, PipeServer};
use huedra_win::process::{self, ExitWatch};
use huedra_win::window::{ProcessHook, Watch};
use huedra_win::{audio, clock, display, foreground, registry, tray, window};

use huedra_core::locations::Locations;
use huedra_win::color::Displays as ColorDisplays;
use huedra_win::stores;

const ENGINE_TIMER: TimerId = TimerId(1);
const FOLLOW_TIMER: TimerId = TimerId(2);
const VIDEO_TIMER: TimerId = TimerId(3);
/// The daily update check; runs only in builds that can update.
const UPDATE_TIMER: TimerId = TimerId(4);
/// Ends early starts whose game never appeared (see `steam_start::GRACE`).
const STEAM_TIMER: TimerId = TimerId(5);
/// The first check waits this long after start, so it does not compete
/// with Windows starting up.
const UPDATE_FIRST_DELAY: Duration = Duration::from_secs(120);
/// How often the process list is read while following a launcher.
const FOLLOW_POLL: Duration = Duration::from_secs(1);
/// How often a video program is read again while SDR video can count
/// (RTX Video HDR): full screen and sound send no window events.
const VIDEO_POLL: Duration = Duration::from_secs(2);
const HISTORY_LEN: usize = 50;
const PAUSE_MINUTES: u32 = 60;
/// Longest pause accepted from a client.
const MAX_PAUSE_MINUTES: u32 = 24 * 60;

const MENU_AUTOMATIC: u32 = 10;
const MENU_ALWAYS_HDR: u32 = 11;
const MENU_ALWAYS_SDR: u32 = 12;
const MENU_NEVER: u32 = 20;
const MENU_PAUSE: u32 = 21;
const MENU_OPEN: u32 = 30;
const MENU_EXIT: u32 = 31;
const MENU_UPDATE: u32 = 32;
const MENU_SUPPORT: u32 = 33;
const MENU_REPORT: u32 = 34;

/// Runs the tray process until it is told to exit, and opens the settings
/// window when the user started it.
pub fn run(start: TrayStart) -> anyhow::Result<()> {
    huedra_win::console::release_own_console();
    let paths = Locations::resolve()?;
    let mut host = EventLoop::new().context("could not create the tray window")?;
    let pipe = match PipeServer::start(host.waker()) {
        Ok(pipe) => pipe,
        Err(PipeError::AlreadyRunning) => {
            // A second start by the user opens the window of the running
            // copy, or brings it to the front, instead.
            eprintln!("Huedra is already running.");
            if start == TrayStart::Window {
                crate::open_settings();
            }
            return Ok(());
        }
        Err(error) => return Err(error).context("could not open Huedra's channel"),
    };
    // Nothing runs, and HDR is never touched, until the disclaimer has been
    // accepted in the settings window, which then starts the tray again.
    let accepted = Config::load(&paths.config).is_ok_and(|c| c.general.disclaimer_accepted);
    if !accepted {
        drop(pipe);
        crate::open_settings();
        return Ok(());
    }
    let mut app = App::start(&host, paths, pipe)?;
    // After an update, the old program waits beside the new one until the
    // process that installed it has ended.
    crate::update::remove_previous();
    if start == TrayStart::Window {
        crate::open_settings();
    }
    host.run(|host, event| app.handle(host, event))?;
    Ok(())
}

struct App {
    paths: Locations,
    config: Config,
    library: Library,
    engine: Engine,
    tracker: Tracker,
    watches: HashMap<Pid, ExitWatch>,
    /// The display showing each tracked game process's main window, when
    /// HDR follows the content; kept while the window is minimized.
    game_monitors: HashMap<Pid, u32>,
    /// Window-move hooks of tracked game processes, so HDR follows a game
    /// moved to another display.
    game_moves: HashMap<Pid, ProcessHook>,
    /// Processes that already ran when a follow window opened; only newer
    /// ones can be a launcher's successor.
    follow_known: HashSet<u32>,
    /// Video players and browsers that came to the front, watched until
    /// they exit.
    videos: HashMap<Pid, VideoProcess>,
    /// Whether NVIDIA RTX Video HDR is on, read when a video program comes
    /// to the front and on reload.
    rtx_video_hdr: bool,
    /// The process in the foreground.
    foreground: u32,
    /// Whether [`VIDEO_TIMER`] runs.
    video_polling: bool,
    /// Running `huedra launch` processes; HDR is held while each lives.
    launches: HashMap<Pid, ExitWatch>,
    /// Steam games that got HDR when Steam started them.
    steam_starts: SteamStarts,
    displays: Vec<Display>,
    /// Color profiles running games ask for, and the changes to undo.
    colors: ColorState,
    color_system: ColorDisplays,
    /// Names of running games that ask for color profiles.
    color_names: HashMap<TriggerId, String>,
    /// The displays each of those games was last seen on, kept while its
    /// window is minimized or a launcher hands over to it.
    color_places: HashMap<TriggerId, BTreeSet<DisplayId>>,
    /// The daily check came due while a game or video used HDR; it runs
    /// once they end.
    update_due: bool,
    /// A newer release waiting for consent (Updates: Ask before
    /// installing), offered in the menu.
    update_available: Option<Version>,
    /// The update dialog is due as soon as no game or video uses HDR.
    offer_update: bool,
    saved_owned: Vec<DisplayId>,
    history: VecDeque<HistoryEntry>,
    log: LogFile,
    tray: tray::Tray,
    hotkeys: Option<Hotkeys>,
    _foreground: Option<foreground::ForegroundHook>,
    pipe: PipeServer,
    waker: Waker,
}

impl App {
    fn start(host: &EventLoop, paths: Locations, pipe: PipeServer) -> anyhow::Result<Self> {
        let log = LogFile::new(paths.log.clone(), logfile::MAX_BYTES);
        let first_run = !paths.config.exists();
        let (config, problem) = match Config::load(&paths.config) {
            Ok(config) => (config, None),
            Err(error) => (Config::default(), Some(error.to_string())),
        };
        let displays = display::current().context("could not read the displays")?;
        let owned = files::load_owned(&paths.owned);
        let now = Instant::now();
        let (engine, outcome) = Engine::new(
            config.mode,
            config.general.hold.duration(),
            &managed(&config, &displays),
            &owned,
            manual_target(&config, &displays),
            now,
        );
        let scan_started = Instant::now();
        let library = build_library(&config);
        let scan_time = scan_started.elapsed();
        let update_available = UpdateState::load(&paths.update).waiting(Version::current());
        let mut app = Self {
            library,
            tray: tray::Tray::new(host).context("could not create the tray icon")?,
            hotkeys: None,
            _foreground: foreground::ForegroundHook::install(),
            tracker: Tracker::default(),
            watches: HashMap::new(),
            game_monitors: HashMap::new(),
            game_moves: HashMap::new(),
            follow_known: HashSet::new(),
            videos: HashMap::new(),
            rtx_video_hdr: registry::rtx_video_hdr() == Some(true),
            foreground: 0,
            video_polling: false,
            launches: HashMap::new(),
            steam_starts: SteamStarts::new(registry::steam_running_app()),
            update_due: false,
            update_available,
            offer_update: false,
            colors: ColorState::from_record(color::load_record(&paths.color)),
            color_system: ColorDisplays::new(paths.color.clone()),
            color_names: HashMap::new(),
            color_places: HashMap::new(),
            saved_owned: owned,
            history: VecDeque::new(),
            waker: host.waker(),
            paths,
            config,
            engine,
            displays,
            log,
            pipe,
        };
        app.register_hotkeys(host);
        // Read only; without Steam there is nothing to watch.
        registry::watch_steam_running_app(host.waker());
        app.apply(host, now, outcome);
        // Profiles left changed by a run that did not end normally go back.
        app.sync_colors();
        if let Some(pid) = foreground::foreground_pid() {
            app.on_foreground(host, now, pid);
        }
        app.update_video_poll(host);
        app.refresh_tray(now);
        let started = huedra_win::clock::since_process_start()
            .map_or_else(String::new, |d| format!(" in {} ms", d.as_millis()));
        app.log_line(&format!(
            "Huedra {} started{started}; game libraries read in {} ms.",
            huedra_core::VERSION,
            scan_time.as_millis()
        ));
        if let Some(problem) = problem {
            app.log_line(&format!(
                "The settings were not loaded and defaults are used: {problem}"
            ));
        }
        app.startup_entry(first_run);
        app.schedule_update(host);
        Ok(app)
    }

    fn handle(&mut self, host: &mut EventLoop, event: Event) -> Flow {
        let now = Instant::now();
        match event {
            Event::Foreground(pid) => self.on_foreground(host, now, pid),
            Event::ProcessExited(pid) => self.on_exit(host, now, Pid(pid)),
            Event::WindowChanged(pid) => {
                let pid = Pid(pid);
                if self.videos.contains_key(&pid) {
                    self.assess_video(host, now, pid);
                } else {
                    self.place_game(host, now, pid);
                }
            }
            Event::Timer(ENGINE_TIMER) => {
                host.kill_timer(ENGINE_TIMER);
                let outcome = self.engine.tick(now);
                self.apply(host, now, outcome);
            }
            Event::Timer(FOLLOW_TIMER) => self.on_follow_tick(host, now),
            Event::Timer(STEAM_TIMER) => self.on_steam_tick(host, now),
            Event::SteamApp(app_id) => self.on_steam_app(host, now, app_id),
            Event::Timer(VIDEO_TIMER) => self.on_video_tick(host, now),
            Event::Timer(UPDATE_TIMER) => {
                // Timers repeat: the next check is a day later.
                host.set_timer(UPDATE_TIMER, update::CHECK_INTERVAL);
                self.update_due = true;
            }
            Event::Timer(_) => {}
            Event::DisplayChanged => self.on_display_change(host, now),
            Event::Hotkey(key) => self.on_hotkey(host, now, key),
            // The quick panel, beside the icon; the menu stays on the
            // right button and is the keyboard and screen reader path too.
            Event::TrayClick => crate::open_flyout(self.tray.rect()),
            // A failure notice: the window shows it on Status.
            Event::NoticeClick => crate::open_settings(),
            Event::TrayMenu => {
                if self.on_menu(host, now) == Flow::Exit {
                    self.shutdown();
                    return Flow::Exit;
                }
            }
            Event::TaskbarCreated => self.tray.restore(),
            Event::Wake => {
                while let Some(call) = self.pipe.next_call() {
                    let (response, flow) = self.on_request(host, now, &call.request);
                    // On exit the displays are back in SDR before the reply,
                    // so "Huedra has closed." is true when it is printed.
                    if flow == Flow::Exit {
                        self.shutdown();
                        call.reply(response);
                        return Flow::Exit;
                    }
                    call.reply(response);
                }
            }
        }
        self.update_video_poll(host);
        self.run_due_update();
        self.offer_waiting_update();
        self.refresh_tray(now);
        Flow::Continue
    }

    // --- Watchers -------------------------------------------------------

    fn on_foreground(&mut self, host: &EventLoop, now: Instant, pid: u32) {
        self.foreground = pid;
        let pid = Pid(pid);
        if self.videos.contains_key(&pid) {
            self.rtx_video_hdr = registry::rtx_video_hdr() == Some(true);
            self.assess_video(host, now, pid);
            return;
        }
        if self.tracker.is_tracked(pid) {
            self.place_game(host, now, pid);
            return;
        }
        let Some(path) = process::image_path(pid.0) else {
            return;
        };
        if self.tracker.is_following() && self.tracker.adopt(pid, &path, now).is_some() {
            if self.watch(pid) {
                self.place_game(host, now, pid);
            }
            return;
        }
        if let Some(app) = VideoApp::from_exe(&path) {
            self.rtx_video_hdr = registry::rtx_video_hdr() == Some(true);
            self.watch_video(host, now, pid, app);
            return;
        }
        let Some(game) = self.library.find(&path).cloned() else {
            return;
        };
        // A game is followed when it changes HDR, uses its own color
        // profiles, or both.
        let trigger = self.config.trigger_for(&game);
        let colors = self.config.color_profiles(&game.id).is_some();
        if trigger.is_none() && !colors {
            return;
        }
        if !self.watch(pid) {
            return;
        }
        let id = TriggerId::new(game.id.as_str());
        if self.tracker.track(pid, id.clone(), &game.install_dir, now) {
            self.find_game_window(pid);
            self.log_line(&format!("{} started.", game.name));
            if let Some(mut trigger) = trigger {
                trigger.target = self.game_target(&id);
                self.follow_steam_start(host, now, &id, &trigger.target);
                let outcome = self.engine.trigger_started(now, id, trigger);
                self.apply(host, now, outcome);
            }
            self.sync_colors();
        } else {
            self.place_game(host, now, pid);
        }
    }

    // --- Displays showing the content -------------------------------------

    /// The displays a trigger wants, given the Windows numbers of the
    /// displays its windows are on: those displays when HDR follows the
    /// content, otherwise every managed display.
    fn target_for(&self, monitors: &BTreeSet<u32>) -> Target {
        if !self.config.displays.content_only {
            return Target::All;
        }
        Target::Only(
            self.displays
                .iter()
                .filter(|d| monitors.contains(&d.number))
                .map(|d| d.id.clone())
                .collect(),
        )
    }

    /// The displays a game wants: those of its processes' main windows.
    fn game_target(&self, id: &TriggerId) -> Target {
        let monitors = self
            .game_monitors
            .iter()
            .filter(|(pid, _)| self.tracker.trigger_of(**pid) == Some(id))
            .map(|(_, monitor)| *monitor)
            .collect();
        self.target_for(&monitors)
    }

    /// Notes the display of a tracked game process's main window, and
    /// watches its window moves. A process without a window keeps the
    /// display it was last seen on.
    fn find_game_window(&mut self, pid: Pid) {
        if !self.config.displays.content_only {
            return;
        }
        if !self.game_moves.contains_key(&pid)
            && let Some(hook) = ProcessHook::install(pid.0, Watch::Moves)
        {
            self.game_moves.insert(pid, hook);
        }
        if let Some(monitor) = window::main_monitor(pid.0) {
            self.game_monitors.insert(pid, monitor);
        }
    }

    /// Moves a running game's HDR to the display its window is on now.
    fn place_game(&mut self, host: &EventLoop, now: Instant, pid: Pid) {
        let Some(id) = self.tracker.trigger_of(pid).cloned() else {
            return;
        };
        self.find_game_window(pid);
        let target = self.game_target(&id);
        self.follow_steam_start(host, now, &id, &target);
        let outcome = self.engine.retarget(now, &id, target);
        if outcome != Outcome::default() {
            self.apply(host, now, outcome);
        }
        if self.color_names.contains_key(&id) {
            self.sync_colors();
        }
    }

    // --- Color profiles ---------------------------------------------------

    /// Brings the displays' color profiles in line with the running games
    /// that ask for their own: sets theirs on the displays they are on, and
    /// puts back the earlier profiles where no game wants a change any
    /// more.
    fn sync_colors(&mut self) {
        // Running games that ask for profiles, including ones given profiles
        // while they run; a game a launcher is handing over to keeps its own.
        let tracked: BTreeSet<TriggerId> = self
            .tracker
            .pids()
            .filter_map(|pid| self.tracker.trigger_of(pid).cloned())
            .collect();
        for id in tracked {
            let game = GameId::from_raw(id.as_str());
            if !self.color_names.contains_key(&id) && self.config.color_profiles(&game).is_some() {
                let name = self
                    .library
                    .get(&game)
                    .map_or_else(|| id.as_str().to_owned(), |g| g.name.clone());
                self.color_names.insert(id, name);
            }
        }
        let tracker = &self.tracker;
        self.color_names.retain(|id, _| tracker.is_active(id));
        self.color_places.retain(|id, _| tracker.is_active(id));
        let ids: BTreeSet<TriggerId> = self.color_names.keys().cloned().collect();
        let mut games = Vec::new();
        for id in ids {
            let Some(profiles) = self.config.color_profiles(&GameId::from_raw(id.as_str())) else {
                continue;
            };
            let displays = self.color_displays(&id);
            let label = self
                .color_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| id.as_str().to_owned());
            games.push(GameColor {
                id,
                label,
                displays,
                profiles,
            });
        }
        if games.is_empty() && !self.colors.is_active() {
            return;
        }
        self.colors.set_games(games);
        for line in self.colors.sync(&mut self.color_system) {
            self.log_line(&line);
        }
        if let Some(problem) = self.color_system.problem.take() {
            self.log_line(&format!(
                "Could not write the record of color profiles: {problem}"
            ));
        }
    }

    /// The displays a game's color profiles go on: those of its processes'
    /// main windows, the ones last seen while it has none showing, or the
    /// main display, where games open by default.
    fn color_displays(&mut self, id: &TriggerId) -> BTreeSet<DisplayId> {
        let numbers: BTreeSet<u32> = self
            .tracker
            .pids()
            .filter(|pid| self.tracker.trigger_of(*pid) == Some(id))
            .filter_map(|pid| window::main_monitor(pid.0))
            .collect();
        let shown: BTreeSet<DisplayId> = self
            .displays
            .iter()
            .filter(|d| numbers.contains(&d.number))
            .map(|d| d.id.clone())
            .collect();
        if !shown.is_empty() {
            self.color_places.insert(id.clone(), shown.clone());
            return shown;
        }
        if let Some(last) = self.color_places.get(id) {
            return last.clone();
        }
        self.displays
            .iter()
            .filter(|d| d.primary)
            .map(|d| d.id.clone())
            .collect()
    }

    /// Applies a change of the Displays setting to running games and
    /// video.
    fn retarget_all(&mut self, host: &EventLoop, now: Instant) {
        let pids: Vec<Pid> = self.tracker.pids().collect();
        for pid in &pids {
            self.find_game_window(*pid);
        }
        if !self.config.displays.content_only {
            self.game_moves.clear();
        }
        let ids: BTreeSet<TriggerId> = pids
            .iter()
            .filter_map(|pid| self.tracker.trigger_of(*pid).cloned())
            .collect();
        for id in ids {
            let outcome = self.engine.retarget(now, &id, self.game_target(&id));
            self.apply(host, now, outcome);
        }
    }

    /// Starts waiting for a process to exit. Without a wait its end could
    /// not be noticed, so such a process is not tracked.
    fn watch(&mut self, pid: Pid) -> bool {
        match ExitWatch::new(pid.0, self.waker) {
            Some(watch) => {
                self.watches.insert(pid, watch);
                true
            }
            None => false,
        }
    }

    // --- Video ----------------------------------------------------------

    /// Starts following a video program's window titles. Programs set to
    /// Never are not watched at all.
    fn watch_video(&mut self, host: &EventLoop, now: Instant, pid: Pid, app: VideoApp) {
        if self.config.video.mode(app) == PlayerHdr::Never {
            return;
        }
        let Some(titles) = ProcessHook::install(pid.0, Watch::Titles) else {
            return;
        };
        let Some(exit) = ExitWatch::new(pid.0, self.waker) else {
            return;
        };
        self.videos.insert(
            pid,
            VideoProcess {
                app,
                _titles: titles,
                moves: None,
                _exit: exit,
                playing: None,
                target: Target::All,
                sdr_full_screen: false,
            },
        );
        self.assess_video(host, now, pid);
    }

    /// Reads a video program's windows again and starts or ends its trigger.
    fn assess_video(&mut self, host: &EventLoop, now: Instant, pid: Pid) {
        let Some(app) = self.videos.get(&pid).map(|v| v.app) else {
            return;
        };
        let assessment =
            self.config
                .video
                .assess(app, self.rtx_video_hdr, &window::windows_of(pid.0));
        // Browsers play sound from helper processes of the same program,
        // so any process of the program counts.
        let sound = assessment.sdr_full_screen
            && audio::playing(|other| {
                process::image_path(other).is_some_and(|p| VideoApp::from_exe(&p) == Some(app))
            });
        let playing = assessment.playing(sound);
        let target = self.target_for(&assessment.monitors);
        let follow = self.config.displays.content_only && playing.is_some();
        let Some(proc) = self.videos.get_mut(&pid) else {
            return;
        };
        // Window moves matter while an HDR title waits for full screen, and
        // while video plays and HDR follows its window to another display.
        match (assessment.candidate || follow, proc.moves.is_some()) {
            (true, false) => proc.moves = ProcessHook::install(pid.0, Watch::Moves),
            (false, true) => proc.moves = None,
            _ => {}
        }
        proc.sdr_full_screen = assessment.sdr_full_screen;
        let id = video::trigger_id(app, pid.0);
        if playing.is_some() == proc.playing.is_some() {
            if playing.is_some() && proc.target != target {
                proc.target = target.clone();
                let outcome = self.engine.retarget(now, &id, target);
                self.apply(host, now, outcome);
            }
            return;
        }
        let previous = std::mem::replace(&mut proc.playing, playing);
        proc.target = target.clone();
        let outcome = match (playing, previous) {
            (Some(kind), _) => {
                let mut trigger = self.config.video_trigger(app, kind);
                trigger.target = target;
                self.log_line(&format!("{} started.", trigger.label));
                self.engine.trigger_started(now, id, trigger)
            }
            (None, Some(kind)) => {
                self.log_line(&format!("{} ended.", video::trigger_label(app, kind)));
                self.engine.trigger_ended(now, &id)
            }
            (None, None) => return,
        };
        self.apply(host, now, outcome);
    }

    /// Reads again the video programs that SDR video could make want HDR:
    /// the one in the foreground and those with a full-screen window.
    fn on_video_tick(&mut self, host: &EventLoop, now: Instant) {
        let pids: Vec<Pid> = self
            .videos
            .iter()
            .filter(|(pid, v)| pid.0 == self.foreground || v.sdr_full_screen || v.playing.is_some())
            .map(|(pid, _)| *pid)
            .collect();
        for pid in pids {
            self.assess_video(host, now, pid);
        }
    }

    /// Runs [`VIDEO_TIMER`] only while SDR video can count and a video
    /// program is in the foreground or keeps SDR video in full screen.
    fn update_video_poll(&mut self, host: &EventLoop) {
        let sdr = self.config.video.sdr_with_rtx && self.rtx_video_hdr;
        let needed = self.videos.iter().any(|(pid, v)| {
            v.sdr_full_screen
                || v.playing == Some(VideoKind::Sdr)
                || (sdr && pid.0 == self.foreground)
        });
        if needed == self.video_polling {
            return;
        }
        self.video_polling = needed;
        if needed {
            host.set_timer(VIDEO_TIMER, VIDEO_POLL);
        } else {
            host.kill_timer(VIDEO_TIMER);
        }
    }

    /// Stops watching a video program and ends its trigger.
    fn forget_video(&mut self, host: &EventLoop, now: Instant, pid: Pid) {
        let Some(proc) = self.videos.remove(&pid) else {
            return;
        };
        if let Some(kind) = proc.playing {
            self.log_line(&format!("{} ended.", video::trigger_label(proc.app, kind)));
            let outcome = self
                .engine
                .trigger_ended(now, &video::trigger_id(proc.app, pid.0));
            self.apply(host, now, outcome);
        }
    }

    fn on_exit(&mut self, host: &EventLoop, now: Instant, pid: Pid) {
        self.forget_video(host, now, pid);
        if self.launches.remove(&pid).is_some() {
            let id = launch::trigger_id(pid.0);
            let label = self.engine.label(&id).unwrap_or("A program").to_owned();
            self.log_line(&format!("{label} ended (started with HDR)."));
            let outcome = self.engine.trigger_ended(now, &id);
            self.apply(host, now, outcome);
        }
        self.watches.remove(&pid);
        self.game_monitors.remove(&pid);
        self.game_moves.remove(&pid);
        match self.tracker.exited(pid, now) {
            Exit::Ended(id) => {
                let label = self
                    .engine
                    .label(&id)
                    .or_else(|| self.color_names.get(&id).map(String::as_str))
                    .unwrap_or(id.as_str())
                    .to_owned();
                self.log_line(&format!("{label} ended."));
                let outcome = self.engine.trigger_ended(now, &id);
                self.apply(host, now, outcome);
                self.end_steam_start(host, now, &id);
                self.sync_colors();
            }
            Exit::Following => {
                self.log_line("A launcher closed; looking for the game it started.");
                self.follow_known = process::pids().into_iter().collect();
                host.set_timer(FOLLOW_TIMER, FOLLOW_POLL);
            }
            Exit::StillRunning | Exit::Unknown => {}
        }
    }

    fn on_follow_tick(&mut self, host: &EventLoop, now: Instant) {
        for pid in process::pids() {
            if !self.follow_known.insert(pid) {
                continue;
            }
            let Some(path) = process::image_path(pid) else {
                continue;
            };
            let pid = Pid(pid);
            if let Some(id) = self.tracker.adopt(pid, &path, now) {
                self.log_line(&format!(
                    "Following {} for {}.",
                    huedra_core::paths::display_name(&path),
                    id.as_str()
                ));
                if self.watch(pid) {
                    self.place_game(host, now, pid);
                } else {
                    // It exited at once; treat it like any other exit.
                    self.on_exit(host, now, pid);
                }
            }
            if !self.tracker.is_following() {
                break;
            }
        }
        let expired = self.tracker.expire(now);
        let any_expired = !expired.is_empty();
        for id in expired {
            let outcome = self.engine.trigger_ended(now, &id);
            self.apply(host, now, outcome);
            self.end_steam_start(host, now, &id);
        }
        if any_expired {
            self.sync_colors();
        }
        if !self.tracker.is_following() {
            host.kill_timer(FOLLOW_TIMER);
            self.follow_known.clear();
        }
    }

    fn on_display_change(&mut self, host: &EventLoop, now: Instant) {
        match display::current() {
            Ok(displays) => self.displays = displays,
            Err(error) => {
                self.log_line(&format!("Could not read the displays: {error}"));
                return;
            }
        }
        let mut outcome = self
            .engine
            .set_displays(now, &managed(&self.config, &self.displays));
        let more = self
            .engine
            .set_manual_target(now, manual_target(&self.config, &self.displays));
        outcome.actions.extend(more.actions);
        outcome.notes.extend(more.notes);
        self.apply(host, now, outcome);
        // A display that came back gets its profiles put back or set.
        self.sync_colors();
    }

    // --- Launch with HDR -------------------------------------------------

    /// Turns HDR on at once for a program `huedra launch` is about to
    /// start, and holds it while that launching process lives. The answer
    /// says whether HDR is on, so the game starts only afterwards.
    fn launch(&mut self, host: &EventLoop, now: Instant, pid: Pid, program: &str) -> Response {
        if self.launches.contains_key(&pid) {
            return Response::Failed {
                message: "This program is already starting.".to_owned(),
            };
        }
        let Some(watch) = ExitWatch::new(pid.0, self.waker) else {
            return Response::Failed {
                message: "Huedra couldn't track the program's start, so HDR stayed off.".to_owned(),
            };
        };
        self.launches.insert(pid, watch);
        let game = self.library.find(program).cloned();
        let label = game
            .as_ref()
            .map_or_else(|| program_label(program), |g| g.name.clone());
        let hold = game
            .as_ref()
            .and_then(|g| self.config.game(&g.id))
            .and_then(|s| s.hold)
            .unwrap_or(self.config.general.hold)
            .duration();
        self.log_line(&format!("{label} is starting with HDR."));
        let trigger = Trigger {
            label,
            delay: Duration::ZERO,
            hold,
            target: self.main_target(),
        };
        let outcome = self
            .engine
            .trigger_started(now, launch::trigger_id(pid.0), trigger);
        self.apply(host, now, outcome);
        let status = self.engine.status(now);
        let message = if status.hdr_on {
            "HDR is on."
        } else if status.mode == Mode::AlwaysSdr {
            "HDR stays off: Huedra is in Always SDR mode."
        } else if status.paused_until.is_some() {
            "HDR stays off: automatic switching is paused."
        } else if status.displays.is_empty() {
            "HDR stays off: no display Huedra manages supports HDR."
        } else {
            "HDR could not be turned on; see Huedra's log."
        };
        Response::Done {
            message: message.to_owned(),
        }
    }

    /// The displays for a game that has no window yet: the main display,
    /// where games open by default (every managed display when HDR does
    /// not follow the content).
    fn main_target(&self) -> Target {
        let main: BTreeSet<u32> = self
            .displays
            .iter()
            .filter(|d| d.primary)
            .map(|d| d.number)
            .collect();
        self.target_for(&main)
    }

    // --- HDR when Steam starts a game ------------------------------------

    /// Steam reports another running game. A game set to "Turn HDR on when
    /// Steam starts it" gets HDR now, before its window shows.
    fn on_steam_app(&mut self, host: &EventLoop, now: Instant, app_id: u32) {
        if let Some(app_id) = self.steam_starts.steam_reports(app_id, now) {
            let trigger = self
                .library
                .get(&steam_start::game_id(app_id))
                .and_then(|game| self.config.steam_start_trigger(game));
            if let Some(mut trigger) = trigger {
                self.steam_starts.started(app_id);
                trigger.target = self.main_target();
                self.log_line(&format!(
                    "Steam is starting {}; HDR turns on before its window shows.",
                    trigger.label
                ));
                let outcome =
                    self.engine
                        .trigger_started(now, steam_start::trigger_id(app_id), trigger);
                self.apply(host, now, outcome);
            }
        }
        self.schedule_steam_tick(host, now);
    }

    /// Ends the early starts of games that Steam stopped reporting and
    /// that never appeared.
    fn on_steam_tick(&mut self, host: &EventLoop, now: Instant) {
        let tracker = &self.tracker;
        let ended = self.steam_starts.expire(now, |app_id| {
            tracker.is_active(&TriggerId::new(steam_start::game_id(app_id).as_str()))
        });
        for app_id in ended {
            let id = steam_start::trigger_id(app_id);
            let label = self.engine.label(&id).unwrap_or("A Steam game").to_owned();
            self.log_line(&format!(
                "{label} did not appear after Steam started it; HDR is no longer kept on for it."
            ));
            let outcome = self.engine.trigger_ended(now, &id);
            self.apply(host, now, outcome);
        }
        self.schedule_steam_tick(host, now);
    }

    /// Sets [`STEAM_TIMER`] for the next grace end, or stops it.
    fn schedule_steam_tick(&self, host: &EventLoop, now: Instant) {
        match self.steam_starts.next_deadline() {
            Some(at) => host.set_timer(STEAM_TIMER, at.saturating_duration_since(now)),
            None => host.kill_timer(STEAM_TIMER),
        }
    }

    /// Moves a game's early start to the displays the game itself wants,
    /// once its window is known; until then it keeps the main display.
    fn follow_steam_start(&mut self, host: &EventLoop, now: Instant, id: &TriggerId, to: &Target) {
        let Some(app_id) = GameId::from_raw(id.as_str()).steam_app_id() else {
            return;
        };
        if !self.steam_starts.is_starting(app_id) || *to == Target::Only(BTreeSet::new()) {
            return;
        }
        let outcome = self
            .engine
            .retarget(now, &steam_start::trigger_id(app_id), to.clone());
        if outcome != Outcome::default() {
            self.apply(host, now, outcome);
        }
    }

    /// The game's own trigger ended: its early start ends with it.
    fn end_steam_start(&mut self, host: &EventLoop, now: Instant, id: &TriggerId) {
        let Some(app_id) = GameId::from_raw(id.as_str()).steam_app_id() else {
            return;
        };
        if self.steam_starts.game_ended(app_id) {
            let outcome = self
                .engine
                .trigger_ended(now, &steam_start::trigger_id(app_id));
            self.apply(host, now, outcome);
            self.schedule_steam_tick(host, now);
        }
    }

    // --- User commands --------------------------------------------------

    fn on_hotkey(&mut self, host: &EventLoop, now: Instant, key: Hotkey) {
        match key {
            Hotkey::ToggleHdr => {
                let outcome = self.engine.toggle(now);
                self.apply(host, now, outcome);
                self.remember_mode();
            }
            Hotkey::CycleMode => self.set_mode(host, now, self.engine.mode().next()),
            Hotkey::Pause => self.toggle_pause(host, now),
        }
    }

    fn set_mode(&mut self, host: &EventLoop, now: Instant, mode: Mode) {
        let outcome = self.engine.set_mode(now, mode);
        self.apply(host, now, outcome);
        self.log_line(&format!("Mode changed to {}.", mode.label()));
        self.remember_mode();
    }

    /// Modes stay in effect until changed, including across restarts.
    fn remember_mode(&mut self) {
        let mode = self.engine.mode();
        if self.config.mode != mode {
            self.edit_config(|c| c.mode = mode);
        }
    }

    fn toggle_pause(&mut self, host: &EventLoop, now: Instant) {
        if self.engine.is_paused(now) {
            self.resume(host, now);
        } else {
            self.pause(host, now, PAUSE_MINUTES);
        }
    }

    fn pause(&mut self, host: &EventLoop, now: Instant, minutes: u32) {
        let until = now + Duration::from_secs(u64::from(minutes) * 60);
        let outcome = self.engine.pause(now, until);
        self.apply(host, now, outcome);
        self.log_line(&format!(
            "Automatic switching paused for {minutes} minutes."
        ));
    }

    fn resume(&mut self, host: &EventLoop, now: Instant) {
        let outcome = self.engine.resume(now);
        self.apply(host, now, outcome);
        self.log_line("Automatic switching resumed.");
    }

    fn never_for(&mut self, host: &EventLoop, now: Instant, id: &TriggerId, label: &str) {
        if let Some(app) = video::app_of(id) {
            self.edit_config(|c| {
                c.video.set_mode(app, PlayerHdr::Never);
            });
            self.log_line(&format!(
                "{}: HDR will no longer be changed for video in this program.",
                app.label()
            ));
            let pids: Vec<Pid> = self
                .videos
                .iter()
                .filter(|(_, v)| v.app == app)
                .map(|(pid, _)| *pid)
                .collect();
            for pid in pids {
                self.forget_video(host, now, pid);
            }
            return;
        }
        self.edit_config(|c| c.game_mut(&GameId::from_raw(id.as_str())).hdr = GameHdr::DoNotChange);
        self.log_line(&format!(
            "{label}: HDR will no longer be changed for this game."
        ));
        let outcome = self.engine.trigger_ended(now, id);
        self.apply(host, now, outcome);
        self.end_steam_start(host, now, id);
    }

    /// What "Never use HDR for" applies to: a game or video program that is
    /// active, not a launch or Steam starting a game (the game's own entry
    /// ends both).
    fn never_target(&self) -> Option<(TriggerId, String)> {
        self.engine
            .active_triggers()
            .into_iter()
            .find(|(id, _)| !launch::is_launch(id) && !steam_start::is_steam_start(id))
    }

    fn on_menu(&mut self, host: &EventLoop, now: Instant) -> Flow {
        let status = self.engine.status(now);
        let mode = status.mode;
        let mut entries = vec![
            MenuEntry::Label(self.state_line(&status)),
            MenuEntry::Label(self.reason_line(&status, now)),
            MenuEntry::Separator,
            radio(
                MENU_AUTOMATIC,
                &format!("Automatic\t{}", Hotkey::CycleMode.label()),
                mode == Mode::Automatic,
            ),
            radio(MENU_ALWAYS_HDR, "Always HDR", mode == Mode::AlwaysHdr),
            radio(MENU_ALWAYS_SDR, "Always SDR", mode == Mode::AlwaysSdr),
            MenuEntry::Separator,
        ];
        let game = self.never_target();
        if let Some((_, label)) = &game {
            entries.push(item(MENU_NEVER, &format!("Never use HDR for {label}")));
        }
        let pause_text = if status.paused_until.is_some() {
            "Resume automatic switching"
        } else {
            "Pause for 1 hour"
        };
        entries.push(item(
            MENU_PAUSE,
            &format!("{pause_text}\t{}", Hotkey::Pause.label()),
        ));
        entries.push(MenuEntry::Separator);
        if let Some(version) = self.update_available {
            entries.push(item(MENU_UPDATE, &format!("Install Huedra {version}...")));
        }
        entries.push(item(MENU_OPEN, "Open Huedra"));
        entries.push(item(MENU_REPORT, "Report a problem..."));
        entries.push(item(MENU_SUPPORT, "Support Huedra on Buy Me a Coffee..."));
        entries.push(item(MENU_EXIT, "Exit"));

        let now = Instant::now(); // the menu may have been open a while
        match menu::show(host, &entries) {
            Some(MENU_AUTOMATIC) => self.set_mode(host, now, Mode::Automatic),
            Some(MENU_ALWAYS_HDR) => self.set_mode(host, now, Mode::AlwaysHdr),
            Some(MENU_ALWAYS_SDR) => self.set_mode(host, now, Mode::AlwaysSdr),
            Some(MENU_NEVER) => {
                if let Some((id, label)) = game {
                    self.never_for(host, now, &id, &label);
                }
            }
            Some(MENU_PAUSE) => self.toggle_pause(host, now),
            Some(MENU_OPEN) => crate::open_settings(),
            // The settings window shows the update dialog: what is new,
            // and Download and install.
            Some(MENU_UPDATE) => crate::open_settings(),
            Some(MENU_REPORT) => {
                if huedra_win::report::copy_details(&self.paths) {
                    self.log_line("Problem report details copied to the clipboard.");
                } else {
                    self.log_line("Could not copy the problem report details.");
                }
            }
            Some(MENU_SUPPORT) => {
                if !huedra_win::shell::open_web_page(update::SUPPORT_URL) {
                    self.log_line("Could not open the Buy Me a Coffee page in the browser.");
                }
            }
            Some(MENU_EXIT) => {
                // Exit closes all of Huedra: the settings window too.
                huedra_win::instance::close_window(huedra_core::APP_NAME);
                return Flow::Exit;
            }
            _ => {}
        }
        Flow::Continue
    }

    fn on_request(
        &mut self,
        host: &EventLoop,
        now: Instant,
        request: &Request,
    ) -> (Response, Flow) {
        let done = |message: &str| Response::Done {
            message: message.to_owned(),
        };
        let response = match request {
            Request::Status => Response::Status(self.report(now)),
            Request::SetMode { mode } => {
                self.set_mode(host, now, *mode);
                done(&format!("The mode is now {}.", mode.label()))
            }
            Request::Pause { minutes } => {
                let minutes = (*minutes).clamp(1, MAX_PAUSE_MINUTES);
                self.pause(host, now, minutes);
                done(&format!(
                    "Automatic switching is paused for {minutes} minutes."
                ))
            }
            Request::Resume => {
                self.resume(host, now);
                done("Automatic switching has resumed.")
            }
            Request::ToggleHdr => {
                self.on_hotkey(host, now, Hotkey::ToggleHdr);
                let on = self.engine.status(now).hdr_on;
                done(if on { "HDR is on." } else { "HDR is off." })
            }
            Request::SetDisplayHdr { display, on } => {
                let state = if *on { HdrState::On } else { HdrState::Off };
                let id = DisplayId::new(display.as_str());
                let outcome = self.engine.set_display(now, &id, state);
                self.apply(host, now, outcome);
                self.remember_mode();
                done(&format!(
                    "HDR is {} on {}.",
                    if *on { "on" } else { "off" },
                    self.display_label(&id)
                ))
            }
            Request::NeverForActive => match self.never_target() {
                Some((id, label)) => {
                    self.never_for(host, now, &id, &label);
                    done(&format!("HDR is no longer changed for {label}."))
                }
                None => Response::Failed {
                    message: "No game or video program is active.".to_owned(),
                },
            },
            Request::Reload => {
                self.reload(host, now);
                done("The settings were reloaded.")
            }
            Request::Launch { pid, program } => self.launch(host, now, Pid(*pid), program),
            Request::UpdateAvailable { version } => match version.parse::<Version>() {
                Ok(version) if version > Version::current() => {
                    self.update_available = Some(version);
                    self.offer_update = true;
                    done(&format!(
                        "Huedra {version} will be offered once no game or video is using HDR."
                    ))
                }
                _ => Response::Failed {
                    message: "That is not a newer version.".to_owned(),
                },
            },
            Request::Exit => {
                // As Exit in the menu: all of Huedra closes, the settings
                // window too, so none is left without a tray behind it.
                huedra_win::instance::close_window(huedra_core::APP_NAME);
                return (done("Huedra has closed."), Flow::Exit);
            }
        };
        (response, Flow::Continue)
    }

    fn reload(&mut self, host: &EventLoop, now: Instant) {
        match Config::load(&self.paths.config) {
            Ok(config) => self.config = config,
            Err(error) => {
                self.log_line(&format!("The settings were not reloaded: {error}"));
                return;
            }
        }
        self.library = build_library(&self.config);
        self.engine
            .set_default_hold(self.config.general.hold.duration());
        self.register_hotkeys(host);
        self.schedule_update(host);
        let owned_before = self.engine.owned();
        let managed_now = managed(&self.config, &self.displays);
        let mut outcome = self.engine.set_displays(now, &managed_now);
        // HDR that Huedra turned on for a display the user has just excluded
        // goes back to SDR; the engine no longer follows that display, so
        // nothing else would ever turn it off.
        for id in owned_before {
            let excluded = !managed_now.iter().any(|(d, _)| *d == id);
            if excluded && self.displays.iter().any(|d| d.id == id) {
                outcome.actions.push(Action {
                    display: id.clone(),
                    state: HdrState::Off,
                });
                outcome.notes.push(Note {
                    display: id,
                    kind: NoteKind::Disabled,
                    reason: "Huedra no longer manages this display".into(),
                });
            }
        }
        let more = self
            .engine
            .set_manual_target(now, manual_target(&self.config, &self.displays));
        outcome.actions.extend(more.actions);
        outcome.notes.extend(more.notes);
        if self.config.mode != self.engine.mode() {
            let more = self.engine.set_mode(now, self.config.mode);
            outcome.actions.extend(more.actions);
            outcome.notes.extend(more.notes);
        }
        self.apply(host, now, outcome);
        self.retarget_all(host, now);
        // A running game's profiles may have changed.
        self.sync_colors();
        self.rtx_video_hdr = registry::rtx_video_hdr() == Some(true);
        let pids: Vec<(Pid, VideoApp)> = self.videos.iter().map(|(p, v)| (*p, v.app)).collect();
        for (pid, app) in pids {
            if self.config.video.mode(app) == PlayerHdr::Never {
                self.forget_video(host, now, pid);
            } else {
                self.assess_video(host, now, pid);
            }
        }
        self.log_line("Settings reloaded.");
    }

    fn register_hotkeys(&mut self, host: &EventLoop) {
        self.hotkeys = None;
        if !self.config.general.hotkeys {
            return;
        }
        let (hotkeys, refused) = Hotkeys::register(host);
        self.hotkeys = Some(hotkeys);
        for key in refused {
            self.log_line(&format!(
                "{} is already used by another program.",
                key.label()
            ));
        }
    }

    /// Puts back the color profiles games changed and turns off the HDR
    /// Huedra turned on, then clears the recovery records.
    fn shutdown(&mut self) {
        if self.colors.is_active() {
            self.colors.set_games(Vec::new());
            for line in self.colors.sync(&mut self.color_system) {
                self.log_line(&line);
            }
        }
        for id in self.engine.owned() {
            if let Err(error) = display::set_hdr(&id, HdrState::Off, self.config.attempts()) {
                self.log_line(&format!(
                    "Could not return a display to SDR on exit: {error}"
                ));
            }
        }
        self.save_owned(&[]);
        self.log_line("Huedra closed.");
    }

    // --- Updates and start-up --------------------------------------------

    /// Whether a game, video or launch keeps HDR on or is about to, or HDR
    /// Huedra turned on is still held: no update may restart Huedra now.
    fn busy(&self) -> bool {
        !self.engine.active_triggers().is_empty()
            || !self.engine.owned().is_empty()
            || !self.colors.games().is_empty()
    }

    /// Sets the daily check's timer from the last check, or stops it when
    /// updates are off or this build cannot update.
    fn schedule_update(&mut self, host: &EventLoop) {
        if !update::is_available() || self.config.general.updates == UpdatePolicy::Off {
            host.kill_timer(UPDATE_TIMER);
            self.update_due = false;
            return;
        }
        let due = UpdateState::load(&self.paths.update).due_in(SystemTime::now());
        host.set_timer(UPDATE_TIMER, due.max(UPDATE_FIRST_DELAY));
    }

    /// Opens the settings window, which shows the update dialog, once a
    /// newer release was found and no game or video uses HDR, so it never
    /// appears over a game.
    fn offer_waiting_update(&mut self) {
        if !self.offer_update || self.busy() {
            return;
        }
        self.offer_update = false;
        crate::open_settings();
    }

    /// Starts `huedra update --auto` once the check is due and nothing uses
    /// HDR. The check runs in its own process, so the network code never
    /// loads into this one.
    fn run_due_update(&mut self) {
        if !self.update_due || self.busy() {
            return;
        }
        self.update_due = false;
        if let Ok(exe) = std::env::current_exe()
            && !process::spawn_detached(&exe, &["update", "--auto"])
        {
            self.log_line("Could not start the update check.");
        }
    }

    /// Adds the start-up entry on the first start, as Start with Windows is
    /// on by default, and points an entry at this program when the one it
    /// names no longer exists (the folder was moved). An entry for this
    /// program from before `--background` gets it, so signing in does not
    /// open the window. Portable copies leave the registry alone.
    fn startup_entry(&mut self, first_run: bool) {
        if self.paths.portable || !self.config.general.start_with_windows {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let command = startup_command(&exe);
        let stale = registry::startup_command().is_some_and(|current| {
            let program = startup_program(&current);
            let missing = !std::path::Path::new(program).is_file();
            let old_form = program.eq_ignore_ascii_case(&exe.display().to_string())
                && current.trim() != command;
            missing || old_form
        });
        if !first_run && !stale {
            return;
        }
        match registry::set_startup(Some(&command)) {
            Ok(()) => self.log_line("Huedra will start with Windows."),
            Err(error) => self.log_line(&format!("Could not add the start-up entry: {error}")),
        }
        if first_run {
            self.edit_config(|_| {});
        }
    }

    // --- Applying decisions ---------------------------------------------

    fn apply(&mut self, host: &EventLoop, now: Instant, mut outcome: Outcome) {
        // A failed switch can produce one more decision; a few rounds suffice.
        for _ in 0..4 {
            self.record(&outcome.notes);
            if outcome.actions.is_empty() {
                break;
            }
            // Record ownership before turning HDR on, so a crash in between
            // still leaves a record of it.
            let mut planned = self.saved_owned.clone();
            for id in self.engine.owned() {
                if !planned.contains(&id) {
                    planned.push(id);
                }
            }
            self.save_owned(&planned);
            self.log_line(&format!(
                "Switch requested {} ms after the event.",
                now.elapsed().as_millis()
            ));
            let mut next = Outcome::default();
            for action in outcome.actions {
                if let Err(error) =
                    display::set_hdr(&action.display, action.state, self.config.attempts())
                {
                    self.log_line(&format!("Could not change HDR: {error}"));
                    let more = self
                        .engine
                        .switch_failed(now, &action.display, action.state);
                    next.actions.extend(more.actions);
                    next.notes.extend(more.notes);
                }
            }
            let owned = self.engine.owned();
            self.save_owned(&owned);
            outcome = next;
        }
        match self.engine.next_deadline() {
            Some(deadline) => host.set_timer(ENGINE_TIMER, deadline.saturating_duration_since(now)),
            None => host.kill_timer(ENGINE_TIMER),
        }
    }

    fn record(&mut self, notes: &[Note]) {
        for note in notes {
            let (title, kind) = match note.kind {
                NoteKind::Enabled => ("HDR turned on", HistoryKind::Hdr),
                NoteKind::Disabled => ("HDR turned off", HistoryKind::Sdr),
                NoteKind::External if note.reason.contains("will leave it on") => {
                    ("Left on", HistoryKind::External)
                }
                NoteKind::External => ("Changed outside Huedra", HistoryKind::External),
                NoteKind::Failed => ("Couldn't switch", HistoryKind::Failed),
            };
            let display = self.display_label(&note.display);
            self.log_line(&format!("{title} ({display}): {}.", note.reason));
            let now = clock::now();
            let time = now.hour_minute();
            // Several displays switching for one reason make one entry.
            let same = self.history.front().is_some_and(|last| {
                last.time == time && last.title == title && last.detail == note.reason
            });
            if !same && kind == HistoryKind::Failed && self.config.general.notify_failures {
                self.tray.notify(
                    &format!("{display} didn't switch"),
                    &format!(
                        "{}. Huedra tried {} more times. Recent changes on the Status page have the details.",
                        note.reason,
                        self.config.general.retry_attempts
                    ),
                );
            }
            if !same {
                self.history.push_front(HistoryEntry {
                    time,
                    day: now.date(),
                    title: title.to_owned(),
                    detail: note.reason.clone(),
                    kind,
                });
                self.history.truncate(HISTORY_LEN);
            }
        }
    }

    fn save_owned(&mut self, owned: &[DisplayId]) {
        if self.saved_owned == owned {
            return;
        }
        match files::save_owned(&self.paths.owned, owned) {
            Ok(()) => self.saved_owned = owned.to_vec(),
            Err(error) => self.log_line(&format!("Could not write the recovery record: {error}")),
        }
    }

    /// Makes one change to the settings and saves them. The file is read
    /// first, so a change saved by the settings window and not yet reloaded
    /// is kept.
    fn edit_config(&mut self, change: impl FnOnce(&mut Config)) {
        if let Ok(config) = Config::load(&self.paths.config) {
            self.config = config;
        }
        change(&mut self.config);
        if let Err(error) = self.config.save(&self.paths.config) {
            self.log_line(&format!("Could not save the settings: {error}"));
        }
    }

    fn log_line(&self, message: &str) {
        let _ = self.log.append(&format!("{} {message}", clock::now()));
    }

    // --- Presentation ---------------------------------------------------

    fn refresh_tray(&mut self, now: Instant) {
        let status = self.engine.status(now);
        let icon = if status.hdr_on {
            TrayIcon::Hdr
        } else {
            TrayIcon::Sdr
        };
        let tip = format!("Huedra\n{}", self.summary(&status, now));
        self.tray.show(icon, &tip);
    }

    /// "HDR on · Cyberpunk 2077", "HDR off · Automatic".
    fn summary(&self, status: &Status, now: Instant) -> String {
        let state = if status.hdr_on { "HDR on" } else { "HDR off" };
        let detail = if let Some(minutes) = paused_minutes(status, now) {
            format!("Paused until {}", clock::now().hour_minute_after(minutes))
        } else if status.mode != Mode::Automatic {
            status.mode.label().to_owned()
        } else if let Some(reason) = &status.reason {
            reason.clone()
        } else {
            Mode::Automatic.label().to_owned()
        };
        format!("{state} · {detail}")
    }

    /// "HDR on, Display 1".
    fn state_line(&self, status: &Status) -> String {
        let numbers: Vec<String> = status
            .displays
            .iter()
            .filter(|(_, phase)| !status.hdr_on || phase_is_on(*phase))
            .filter_map(|(id, _)| self.display(id).map(|d| d.number.to_string()))
            .collect();
        let state = if status.hdr_on { "HDR on" } else { "HDR off" };
        match numbers.as_slice() {
            [] => state.to_owned(),
            [one] => format!("{state}, Display {one}"),
            many => format!("{state}, Displays {}", many.join(" and ")),
        }
    }

    /// "Cyberpunk 2077 is running", "Nothing needs HDR right now".
    fn reason_line(&self, status: &Status, now: Instant) -> String {
        if let Some(minutes) = paused_minutes(status, now) {
            format!("Paused until {}", clock::now().hour_minute_after(minutes))
        } else if status.mode != Mode::Automatic {
            format!("{} mode", status.mode.label())
        } else if let Some(reason) = &status.reason {
            format!("{reason} is running")
        } else {
            "Nothing needs HDR right now".to_owned()
        }
    }

    fn report(&self, now: Instant) -> StatusReport {
        let status = self.engine.status(now);
        StatusReport {
            mode: status.mode,
            paused_minutes: paused_minutes(&status, now),
            hdr_on: status.hdr_on,
            summary: self.summary(&status, now),
            reason: self.reason_line(&status, now),
            active_games: self
                .engine
                .active_triggers()
                .into_iter()
                .map(|(id, _)| id.as_str().to_owned())
                .collect(),
            busy: self.busy(),
            displays: status
                .displays
                .iter()
                .map(|(id, phase)| DisplayReport {
                    id: id.as_str().to_owned(),
                    label: self.display_label(id),
                    hdr_on: phase_is_on(*phase),
                    detail: phase_detail(*phase, now),
                })
                .collect(),
            history: self.history.iter().cloned().collect(),
            never: self
                .never_target()
                .map(|(_, label)| label)
                .unwrap_or_default(),
        }
    }

    fn display(&self, id: &DisplayId) -> Option<&Display> {
        self.displays.iter().find(|d| &d.id == id)
    }

    /// "Display 1 (MO27Q3)".
    fn display_label(&self, id: &DisplayId) -> String {
        match self.display(id) {
            Some(d) if !d.name.trim().is_empty() => format!("Display {} ({})", d.number, d.name),
            Some(d) => format!("Display {}", d.number),
            None => "a display".to_owned(),
        }
    }
}

/// A video player or browser being watched.
struct VideoProcess {
    app: VideoApp,
    _titles: ProcessHook,
    /// Present only while an HDR title waits for full screen.
    moves: Option<ProcessHook>,
    _exit: ExitWatch,
    /// What its running trigger is for; `None` when it has none.
    playing: Option<VideoKind>,
    /// The displays its running trigger wants.
    target: Target,
    /// Whether SDR video can count and a window is full screen, so it is
    /// read again on [`VIDEO_TIMER`].
    sdr_full_screen: bool,
}

/// A program's name for the log and the menu when it is not a known game:
/// its file name without `.exe`.
fn program_label(program: &str) -> String {
    let name = huedra_core::paths::display_name(program);
    let len = name.len();
    if len > 4 && name.is_char_boundary(len - 4) && name[len - 4..].eq_ignore_ascii_case(".exe") {
        name[..len - 4].to_owned()
    } else {
        name.to_owned()
    }
}

/// The displays Huedra manages: HDR-capable and not excluded.
fn managed(config: &Config, displays: &[Display]) -> Vec<(DisplayId, HdrState)> {
    displays
        .iter()
        .filter(|d| d.support.is_capable() && config.is_managed(d.id.as_str()))
        .map(|d| (d.id.clone(), d.state))
        .collect()
}

/// The displays Always HDR mode and the shortcut turn on: every managed
/// display, or only the main one — the Windows primary display, or the
/// lowest-numbered managed display when the primary one is not managed.
fn manual_target(config: &Config, displays: &[Display]) -> Target {
    if config.displays.manual == ManualDisplays::All {
        return Target::All;
    }
    let managed: Vec<&Display> = displays
        .iter()
        .filter(|d| d.support.is_capable() && config.is_managed(d.id.as_str()))
        .collect();
    let main = managed
        .iter()
        .find(|d| d.primary)
        .or_else(|| managed.iter().min_by_key(|d| d.number));
    Target::Only(main.map(|d| d.id.clone()).into_iter().collect())
}

/// Games added by hand come first, so their settings win over a store entry
/// with the same id.
fn build_library(config: &Config) -> Library {
    let mut games = config.manual_games();
    games.extend(stores::scan());
    Library::new(games)
}

fn paused_minutes(status: &Status, now: Instant) -> Option<u32> {
    status.paused_until.map(|until| {
        let secs = until.saturating_duration_since(now).as_secs();
        u32::try_from(secs.div_ceil(60)).unwrap_or(u32::MAX)
    })
}

fn phase_is_on(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::HdrByHuedra | Phase::Holding { .. } | Phase::HdrExternal
    )
}

fn phase_detail(phase: Phase, now: Instant) -> String {
    match phase {
        Phase::Sdr => "HDR off".to_owned(),
        Phase::Waiting { .. } => "HDR off; turning on after the delay".to_owned(),
        Phase::HdrByHuedra => "HDR on (by Huedra)".to_owned(),
        Phase::Holding { until } => {
            let secs = until.saturating_duration_since(now).as_secs().max(1);
            let unit = if secs == 1 { "second" } else { "seconds" };
            format!("HDR on; turns off in {secs} {unit}")
        }
        Phase::HdrExternal => "HDR on (not by Huedra)".to_owned(),
    }
}

fn item(id: u32, text: &str) -> MenuEntry {
    MenuEntry::Item {
        id,
        text: text.to_owned(),
        checked: false,
    }
}

fn radio(id: u32, text: &str, selected: bool) -> MenuEntry {
    MenuEntry::Radio {
        id,
        text: text.to_owned(),
        selected,
    }
}
