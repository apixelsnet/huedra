//! The settings window: loads the settings, games and displays, keeps the
//! Status page in step with the tray process, and saves every change at once
//! (then asks the tray process to reload). Feature doc:
//! `docs/features/08-settings-window.md`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use huedra_core::command;
use huedra_core::config::{
    CloseAction, Config, GameHdr, GameSettings, ManualDisplays, Seconds, UpdatePolicy,
};
use huedra_core::display::{Display, HdrState, HdrSupport};
use huedra_core::engine::{Mode, TriggerId};
use huedra_core::gameinfo::{Cache, GameInfo};
use huedra_core::geometry::WindowGeometry;
use huedra_core::icon;
use huedra_core::ipc::{HistoryKind, Request, Response, StatusReport};
use huedra_core::launch;
use huedra_core::library::{self, Game, GameId, Store};
use huedra_core::locations::Locations;
use huedra_core::paths::{NormPath, display_name};
use huedra_core::video::{self, StreamingSite, VideoApp};
use huedra_win::{instance, pipe, registry, shell};
use slint::{
    CloseRequestResponse, ComponentHandle, Image, Model, ModelRc, PhysicalPosition, PhysicalSize,
    Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};

use crate::icons;
use crate::lookups::{self, Lookup};
#[cfg(feature = "sample")]
use crate::sample;
use crate::ui::{
    DisplayRow, GameDetail, GameRow, HistoryRow, NowRow, SettingsWindow, SiteRow, StoreChip, Theme,
    UpdateNote, VideoAppRow,
};

/// The window title; the tray closes the window by it (Exit in its menu).
const TITLE: &str = huedra_core::APP_NAME;
/// Held by the process showing the settings window, so only one opens.
const WINDOW_CLAIM: &str = "Huedra-settings-window";
/// How long a second start waits for the first one's window to appear
/// before bringing it to the front: a while after sign-in, when the PC is
/// busy and the first window is slow to open.
const WAIT_FOR_WINDOW: Duration = Duration::from_secs(10);
/// The first Windows build with the Mica backdrop for apps (22H2).
const MICA_BUILD: u32 = 22_621;
/// How often the Status page asks the tray process for its state.
const STATUS_POLL: Duration = Duration::from_secs(1);
const SPECTRUM_W: u32 = 640;
const SPECTRUM_H: u32 = 160;

/// The page the window opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    /// Status (the default).
    #[default]
    Status = 0,
    /// Games.
    Games = 1,
    /// Video players.
    Video = 2,
    /// Displays.
    Displays = 3,
    /// Settings (named General before 1.2).
    Settings = 4,
}

impl Page {
    /// Parses a page name as given on the command line. `general`, the
    /// Settings page's earlier name, still works, so shortcuts made for it
    /// keep opening the page.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "status" => Some(Self::Status),
            "games" => Some(Self::Games),
            "video" => Some(Self::Video),
            "displays" => Some(Self::Displays),
            "settings" | "general" => Some(Self::Settings),
            _ => None,
        }
    }
}
const HDR_SETTINGS: [GameHdr; 3] = [
    GameHdr::EnableWhileRunning,
    GameHdr::DoNotChange,
    GameHdr::GameControls,
];

struct State {
    locations: Locations,
    config: Config,
    games: Vec<Game>,
    info: Cache,
    displays: Vec<Display>,
    running: Vec<String>,
    icons: HashMap<GameId, Image>,
    search: String,
    filter: String,
    selected: Option<GameId>,
    /// Result of the last desktop shortcut made, for the game it was made for.
    launch_note: Option<(GameId, String)>,
    /// The version the update dialog was last shown for in this window.
    update_shown: Option<huedra_core::update::Version>,
    /// The failed switch whose notice was closed (its day, time and reason).
    failure_closed: Option<String>,
    /// The failed switch shown in the notice, if any.
    failure: Option<String>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    /// Set once an update is installed: the window opens again on the new
    /// version after it closes.
    static REOPEN: Cell<bool> = const { Cell::new(false) };
}

/// How many changes the update dialog lists; the rest are in the changelog.
const UPDATE_DIALOG_ITEMS: usize = 6;

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with_borrow_mut(|state| state.as_mut().map(f))
}

/// Opens the settings window and runs it until it is closed. If a settings
/// window is already open, brings that one to the front instead.
///
/// # Errors
/// Slint platform errors (no graphics or window system).
pub fn run(locations: Locations, page: Page) -> Result<(), slint::PlatformError> {
    let Some(claim) = instance::claim_window(WINDOW_CLAIM) else {
        // Another process shows the window, or is opening it.
        bring_forward_when_open();
        return Ok(());
    };
    if instance::activate_window(TITLE) {
        return Ok(());
    }
    // The software renderer needs a tenth of the memory of OpenGL and starts
    // faster; this window has nothing it cannot draw.
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into());
    // Mica: Windows tints the navigation pane with the wallpaper, as in its
    // own apps. Windows 11 22H2 (build 22621) and later; earlier builds
    // would show the transparent pane black. Sample builds leave it out
    // unless asked (`HUEDRA_SAMPLE_MICA`): captures of a hidden window
    // cannot show it.
    let mica = registry::windows_build().is_some_and(|build| build >= MICA_BUILD)
        && (!cfg!(feature = "sample") || std::env::var_os("HUEDRA_SAMPLE_MICA").is_some());
    let selector = if mica {
        selector.with_winit_window_attributes_hook(|attributes| {
            use slint::winit_030::winit::platform::windows::{
                BackdropType, WindowAttributesExtWindows as _,
            };
            attributes
                .with_transparent(true)
                .with_system_backdrop(BackdropType::MainWindow)
        })
    } else {
        selector
    };
    selector.select()?;
    let ui = SettingsWindow::new()?;
    ui.set_mica(mica);
    let theme = ui.global::<Theme>();
    theme.set_motion(shell::animations_enabled());
    theme.set_spectrum_image(rgba_image(
        &icon::spectrum_strip(SPECTRUM_W, SPECTRUM_H),
        SPECTRUM_W,
        SPECTRUM_H,
    ));
    ui.set_light_theme(registry::apps_use_light_theme());
    let weak = ui.as_weak();
    registry::watch_light_theme(move |light| {
        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_light_theme(light));
    });
    // A sample window can be shown in either theme without changing Windows.
    #[cfg(feature = "sample")]
    if let Ok(theme) = std::env::var("HUEDRA_SAMPLE_THEME") {
        ui.set_light_theme(theme == "light");
    }
    ui.set_page(page as i32);
    ui.set_version(huedra_core::VERSION.into());
    ui.set_update_note(update_note(&locations.update).into());
    #[cfg(feature = "sample")]
    ui.set_data_folder(sample::DATA_FOLDER.into());
    #[cfg(not(feature = "sample"))]
    ui.set_data_folder(
        locations
            .config
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
            .into(),
    );

    let config = Config::load(&locations.config).unwrap_or_default();
    ui.set_disclaimer_accepted(config.general.disclaimer_accepted);
    ui.set_show_disclaimer(!config.general.disclaimer_accepted);
    ui.set_can_check_updates(!cfg!(feature = "sample") && huedra_core::update::is_available());
    ui.set_has_project_page(huedra_core::update::project_page().is_some());
    ui.set_install_version(install_version(&locations.update).into());
    // A sample window can show the update notice for a made-up version.
    #[cfg(feature = "sample")]
    if let Ok(version) = std::env::var("HUEDRA_SAMPLE_UPDATE") {
        ui.set_install_version(version.into());
    }
    #[cfg(feature = "sample")]
    let info = {
        sample::start(locations.config.clone(), huedra_win::clock::now());
        sample::info()
    };
    #[cfg(not(feature = "sample"))]
    let info = Cache::load(&locations.game_info);
    let games = scan_games(&config);
    let state = State {
        info,
        displays: read_displays().unwrap_or_default(),
        games,
        config,
        locations,
        running: Vec::new(),
        icons: HashMap::new(),
        search: String::new(),
        filter: "all".into(),
        selected: None,
        launch_note: None,
        update_shown: None,
        failure_closed: None,
        failure: None,
    };
    STATE.set(Some(state));

    with_state(|s| {
        show_games(&ui, s);
        show_video(&ui, s);
        show_displays(&ui, s);
        show_settings(&ui, s);
        show_status(&ui, s, None);
    });
    wire_status(&ui);
    wire_games(&ui);
    wire_video(&ui);
    wire_displays(&ui);
    wire_settings(&ui);
    wire_about(&ui);
    restore_geometry(&ui);
    start_status_polling(&ui);
    start_lookups(&ui);
    start_icons(&ui);

    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            // While Download and install runs, the updater closes the tray,
            // which closes this window too; it stays, because it opens Huedra
            // again on the new version when the install is done.
            if ui.get_installing_update() {
                return CloseRequestResponse::KeepWindowShown;
            }
            save_geometry(&ui);
        }
        // "Closing this window: Quit Huedra" closes the tray as well, which
        // returns the displays it turned to HDR to SDR.
        if with_state(|s| s.config.general.on_close) == Some(CloseAction::Quit) {
            let _ = tray(&Request::Exit);
        }
        CloseRequestResponse::HideWindow
    });
    limit_size_once_shown(0);
    log_start_time();
    offer_update(&ui, false);
    ui.run()?;
    drop(ui);
    // The window is gone: a new one may open from here on, the reopened one
    // below too.
    drop(claim);
    // After an update the old window is gone, so the new version opens its
    // own instead of bringing this one to the front.
    if REOPEN.get()
        && let Ok(exe) = std::env::current_exe()
    {
        huedra_win::process::spawn_detached(&exe, &["settings"]);
    }
    Ok(())
}

/// Brings the window another process opens to the front once it exists.
fn bring_forward_when_open() {
    let deadline = std::time::Instant::now() + WAIT_FOR_WINDOW;
    while !instance::activate_window(TITLE) && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
}

/// Closes the window because the tray process has ended: Exit in the tray
/// menu, `huedra exit`, the updater, or a crash. Nothing of Huedra stays
/// open without the tray behind it. The window running Download and install
/// stays, as it opens Huedra again on the new version.
fn close_with_tray(ui: &SettingsWindow) {
    if ui.get_installing_update() {
        return;
    }
    save_geometry(ui);
    let _ = ui.hide();
    let _ = slint::quit_event_loop();
}

/// Logs how long the window took to open, measured from the process's
/// creation to the first turn of its event loop (the window is shown by
/// then; its first frame is drawn on the same turn or the next).
fn log_start_time() {
    let Some(path) = with_state(|s| s.locations.log.clone()) else {
        return;
    };
    slint::Timer::single_shot(Duration::ZERO, move || {
        if let Some(elapsed) = huedra_win::clock::since_process_start() {
            let log = huedra_core::logfile::LogFile::new(path, huedra_core::logfile::MAX_BYTES);
            let _ = log.append(&format!(
                "{} Settings window opened in {} ms.",
                huedra_win::clock::now(),
                elapsed.as_millis()
            ));
        }
    });
}

/// Slint's software renderer works in 16-bit pixel coordinates and stops
/// at a larger window; this stays well below that.
const MAX_WINDOW_PIXELS: u16 = 16_384;

/// Limits the window's size and gives it the program icon once the event
/// loop has created it (Slint does not pass its `icon` on to the window).
fn limit_size_once_shown(attempt: u32) {
    slint::Timer::single_shot(Duration::from_millis(50), move || {
        if instance::limit_window_size(MAX_WINDOW_PIXELS) == 0 && attempt < 40 {
            limit_size_once_shown(attempt + 1);
        }
    });
}

pub(crate) fn rgba_image(rgba: &[u8], width: u32, height: u32) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        rgba, width, height,
    ))
}

// ---- Saving ---------------------------------------------------------

/// Makes one change to the settings, saves them and asks the tray process to
/// read them again. The file is read first: the tray process writes it too
/// (the mode, for one), and saving the copy loaded when the window opened
/// would undo those changes.
fn edit(state: &mut State, change: impl FnOnce(&mut Config)) {
    if let Ok(config) = Config::load(&state.locations.config) {
        state.config = config;
    }
    change(&mut state.config);
    if state.config.save(&state.locations.config).is_ok() {
        thread::spawn(|| {
            let _ = tray(&Request::Reload);
        });
    }
}

// ---- Status -----------------------------------------------------------

fn wire_status(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    ui.on_set_mode(move |index| {
        let mode = match index {
            1 => Mode::AlwaysHdr,
            2 => Mode::AlwaysSdr,
            _ => Mode::Automatic,
        };
        // Shown at once, so arrow keys pressed in quick succession move on
        // from the new mode; the next status poll confirms it.
        if let Some(ui) = weak.upgrade() {
            ui.set_mode(index);
        }
        send(Request::SetMode { mode });
    });
    let weak = ui.as_weak();
    ui.on_toggle_pause(move || {
        let paused = weak.upgrade().is_some_and(|ui| ui.get_paused());
        send(if paused {
            Request::Resume
        } else {
            Request::Pause { minutes: 60 }
        });
    });
    let weak = ui.as_weak();
    ui.on_close_failure(move || {
        if let Some(ui) = weak.upgrade() {
            with_state(|s| {
                s.failure_closed = s.failure.take();
            });
            ui.set_failure("".into());
        }
    });
    // This window is open already, so Huedra starts in the background.
    ui.on_start_huedra(|| {
        if let Ok(exe) = std::env::current_exe() {
            huedra_win::process::spawn_detached(&exe, &[command::BACKGROUND]);
        }
    });
}

/// Asks the tray process. With the `sample` feature the simulated tray
/// answers instead, so a sample window never reaches a real tray.
pub(crate) fn tray(request: &Request) -> Result<Response, pipe::PipeError> {
    #[cfg(feature = "sample")]
    {
        if *request == Request::Status {
            return sample::status()
                .map(Response::Status)
                .ok_or(pipe::PipeError::NotRunning);
        }
        sample::handle(request);
        Ok(Response::Done {
            message: String::new(),
        })
    }
    #[cfg(not(feature = "sample"))]
    pipe::call(request)
}

/// Every display with its state now.
pub(crate) fn read_displays() -> Option<Vec<Display>> {
    #[cfg(feature = "sample")]
    return Some(sample::displays());
    #[cfg(not(feature = "sample"))]
    huedra_win::display::displays().ok()
}

/// The games of every store and those added by hand.
fn scan_games(config: &Config) -> Vec<Game> {
    #[cfg(feature = "sample")]
    let found = sample::games();
    #[cfg(not(feature = "sample"))]
    let found = huedra_win::stores::scan();
    let mut games = config.manual_games();
    games.extend(found);
    library::without_repeats(games)
}

/// Sends a request without blocking the window; the next status poll shows
/// the result.
fn send(request: Request) {
    thread::spawn(move || {
        let _ = tray(&request);
    });
}

fn start_status_polling(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    let _ = thread::Builder::new()
        .name("huedra-status".into())
        .spawn(move || {
            // Whether the tray process has answered since the window opened.
            // A window opened while Huedra was not running stays open, so
            // Start Huedra can be pressed; once the tray has run, the window
            // closes when it ends.
            let mut tray_seen = false;
            loop {
                let answer = tray(&Request::Status);
                // Only a missing pipe means the tray has ended; a busy or
                // failed call does not.
                let tray_gone = matches!(answer, Err(pipe::PipeError::NotRunning));
                let report = match answer {
                    Ok(Response::Status(report)) => Some(report),
                    _ => None,
                };
                // Read here, off the window's thread: every display with its
                // state now, for the Status and Displays pages.
                let displays = read_displays();
                tray_seen |= report.is_some();
                let close = tray_seen && tray_gone;
                let delivered = weak.upgrade_in_event_loop(move |ui| {
                    if close {
                        close_with_tray(&ui);
                    }
                    with_state(|s| {
                        if let Some(displays) = displays
                            && displays != s.displays
                        {
                            s.displays = displays;
                            show_displays(&ui, s);
                        }
                        show_status(&ui, s, report.as_ref());
                    });
                    offer_update(&ui, false);
                });
                if delivered.is_err() {
                    return;
                }
                thread::sleep(STATUS_POLL);
            }
        });
}

fn show_status(ui: &SettingsWindow, state: &mut State, report: Option<&StatusReport>) {
    ui.set_tray_running(report.is_some());
    let Some(report) = report else {
        let on = state.displays.iter().any(|d| d.state == HdrState::On);
        ui.set_hdr_on(on);
        ui.set_headline((if on { "HDR is on" } else { "SDR" }).into());
        ui.set_nav_detail("Huedra is not running".into());
        ui.set_detail("Automatic switching is off while Huedra is not running.".into());
        ui.set_caption(caption(state, on).into());
        ui.set_history(ModelRc::default());
        ui.set_now(ModelRc::default());
        ui.set_idle("Huedra is not running.".into());
        ui.set_failure("".into());
        set_running(ui, state, Vec::new());
        return;
    };
    let on_labels: Vec<&str> = report
        .displays
        .iter()
        .filter(|d| d.hdr_on)
        .map(|d| d.label.as_str())
        .collect();
    let all_labels: Vec<&str> = report.displays.iter().map(|d| d.label.as_str()).collect();
    let detail = if report.hdr_on {
        format!("{}. {}.", join(&on_labels), report.reason)
    } else if all_labels.is_empty() {
        "No connected display supports HDR.".to_owned()
    } else {
        format!("{}. {}.", join(&all_labels), report.reason)
    };
    ui.set_hdr_on(report.hdr_on);
    ui.set_headline((if report.hdr_on { "HDR is on" } else { "SDR" }).into());
    ui.set_nav_detail(nav_detail(&on_labels, report.hdr_on, &report.reason).into());
    ui.set_detail(detail.into());
    ui.set_caption(caption(state, report.hdr_on).into());
    ui.set_mode(match report.mode {
        Mode::Automatic => 0,
        Mode::AlwaysHdr => 1,
        Mode::AlwaysSdr => 2,
    });
    ui.set_paused(report.paused_minutes.is_some());
    ui.set_pause_note(
        report
            .paused_minutes
            .map(|m| {
                format!(
                    "Paused for {m} more {}.",
                    if m == 1 { "minute" } else { "minutes" }
                )
            })
            .unwrap_or_default()
            .into(),
    );
    let today = huedra_win::clock::now().date();
    let mut last_day: Option<&str> = None;
    let history: Vec<HistoryRow> = report
        .history
        .iter()
        .map(|h| HistoryRow {
            group: if last_day.replace(h.day.as_str()) == Some(h.day.as_str()) {
                String::new()
            } else {
                huedra_core::clock::day_label(&h.day, &today)
            }
            .into(),
            time: h.time.as_str().into(),
            title: h.title.as_str().into(),
            detail: h.detail.as_str().into(),
            kind: match h.kind {
                HistoryKind::Hdr => "hdr",
                HistoryKind::Sdr => "sdr",
                HistoryKind::External => "external",
                HistoryKind::Failed => "failed",
            }
            .into(),
        })
        .collect();
    ui.set_history(ModelRc::new(VecModel::from(history)));
    // The newest change is a failed switch: a notice until it is closed.
    state.failure = report
        .history
        .first()
        .filter(|h| h.kind == HistoryKind::Failed)
        .map(|h| format!("{} {} {}", h.day, h.time, h.detail));
    let failure = report
        .history
        .first()
        .filter(|_| state.failure.is_some() && state.failure != state.failure_closed)
        .map(|h| sentence(&h.detail))
        .unwrap_or_default();
    ui.set_failure(failure.into());
    ui.set_now(ModelRc::new(VecModel::from(now_rows(state, report))));
    ui.set_idle(report.reason.as_str().into());
    set_running(ui, state, report.active_games.clone());
}

/// A reason as a sentence: capitalized, with a full stop.
fn sentence(text: &str) -> String {
    let mut chars = text.trim().chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out: String = first.to_uppercase().chain(chars).collect();
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// The games and players holding HDR, for the Now running card.
fn now_rows(state: &State, report: &StatusReport) -> Vec<NowRow> {
    let mut rows = Vec::new();
    for id in &report.active_games {
        if let Some(game) = state.games.iter().find(|g| g.id.as_str() == id) {
            let setting = setting_of(state, &game.id);
            let custom = state
                .config
                .game(&game.id)
                .is_some_and(|s| s.start_delay.is_some() || s.hold.is_some());
            rows.push(NowRow {
                kind: "game".into(),
                id: game.id.as_str().into(),
                name: game.name.as_str().into(),
                icon: state.icons.get(&game.id).cloned().unwrap_or_default(),
                initial: initial(&game.name).into(),
                detail: format!(
                    "{} · {}{}",
                    store_label(game.store),
                    setting.label(),
                    if custom { " · own times" } else { "" }
                )
                .into(),
            });
        } else if let Some(app) = video::app_of(&TriggerId::new(id.as_str())) {
            if rows
                .iter()
                .any(|r| r.kind == "video" && r.name == app.label())
            {
                continue;
            }
            rows.push(NowRow {
                kind: "video".into(),
                id: app.key().into(),
                name: app.label().into(),
                icon: Image::default(),
                initial: SharedString::default(),
                detail: "HDR video".into(),
            });
        }
    }
    rows
}

/// A store's name as the Games page shows it.
fn store_label(store: Store) -> &'static str {
    if store == Store::Manual {
        "Added"
    } else {
        store.label()
    }
}

/// The line under the state in the navigation pane, for example
/// "Display 1 · Always HDR mode" or "No HDR content".
fn nav_detail(on_labels: &[&str], hdr_on: bool, reason: &str) -> String {
    if !hdr_on || on_labels.is_empty() {
        return reason.to_owned();
    }
    // "Display 1 (MO27Q3)" is shortened to "Display 1".
    let names: Vec<&str> = on_labels
        .iter()
        .map(|l| l.split(" (").next().unwrap_or(l))
        .collect();
    format!("{} · {reason}", join(&names))
}

fn set_running(ui: &SettingsWindow, state: &mut State, running: Vec<String>) {
    if state.running != running {
        state.running = running;
        show_games(ui, state);
        show_video(ui, state);
    }
}

fn caption(state: &State, hdr_on: bool) -> String {
    if !hdr_on {
        return "sRGB color · standard brightness".to_owned();
    }
    let peak = state
        .displays
        .iter()
        .filter(|d| d.state == HdrState::On)
        .filter_map(|d| d.peak)
        .map(|n| n.get())
        .fold(0.0_f32, f32::max);
    if peak > 0.0 {
        format!("BT.2020 color · up to {peak:.0} nits")
    } else {
        "BT.2020 color".to_owned()
    }
}

fn join(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

// ---- Games ------------------------------------------------------------

fn setting_of(state: &State, id: &GameId) -> GameHdr {
    state.config.game(id).map(|s| s.hdr).unwrap_or_default()
}

/// What the list shows about a game, used for its row and the filters.
struct Cues {
    /// HDR support: hdr, limited, no, mod, unknown, pending, or empty.
    hdr: &'static str,
    /// Whether it has its own setting (not the default).
    own: bool,
    /// The suggested setting where it differs from the current one.
    suggestion: Option<GameHdr>,
}

fn cues(state: &State, game: &Game) -> Cues {
    use huedra_core::gameinfo::WikiHdr;
    let setting = setting_of(state, &game.id);
    let info = state
        .config
        .general
        .game_info
        .then(|| state.info.get(&game.id))
        .flatten();
    let hdr = if !state.config.general.game_info || game.store == Store::Manual {
        ""
    } else {
        match info {
            None => "pending",
            Some(i) if i.page.is_none() => "unknown",
            Some(i) => match i.hdr {
                WikiHdr::Supported => "hdr",
                WikiHdr::Limited => "limited",
                WikiHdr::NotSupported => "no",
                WikiHdr::Hackable => "mod",
                WikiHdr::Unknown => "unknown",
            },
        }
    };
    Cues {
        hdr,
        own: setting != GameHdr::default(),
        suggestion: info
            .and_then(|i| i.hdr.suggestion())
            .filter(|s| *s != setting),
    }
}

/// Whether a game passes the chosen filter: a store, or one of the cues.
fn passes(filter: &str, game: &Game, cues: &Cues) -> bool {
    match filter {
        "all" => true,
        "hdr" => matches!(cues.hdr, "hdr" | "limited"),
        "own" => cues.own,
        "suggest" => cues.suggestion.is_some(),
        store => game.store.key() == store,
    }
}

/// One of six tints for a game's tile, the same for a name every time.
fn tint(name: &str) -> i32 {
    let sum = name
        .bytes()
        .fold(0_u32, |acc, b| acc.wrapping_add(u32::from(b)));
    i32::try_from(sum % 6).unwrap_or(0)
}

fn info_text(state: &State, id: &GameId) -> String {
    if !state.config.general.game_info {
        return "Game information is turned off".to_owned();
    }
    match state.info.get(id) {
        None => "Not yet looked up".to_owned(),
        Some(info) if info.page.is_none() => "Not found on PCGamingWiki".to_owned(),
        Some(info) => info.hdr.label().to_owned(),
    }
}

fn show_games(ui: &SettingsWindow, state: &State) {
    let search = state.search.to_lowercase();
    let all: Vec<(&Game, Cues)> = state.games.iter().map(|g| (g, cues(state, g))).collect();
    let mut rows: Vec<GameRow> = all
        .iter()
        .filter(|(g, c)| passes(&state.filter, g, c))
        .filter(|(g, _)| search.is_empty() || g.name.to_lowercase().contains(&search))
        .map(|(g, c)| {
            let running = state.running.iter().any(|r| r == g.id.as_str());
            let store = store_label(g.store);
            GameRow {
                id: g.id.as_str().into(),
                name: g.name.as_str().into(),
                icon: state.icons.get(&g.id).cloned().unwrap_or_default(),
                initial: initial(&g.name).into(),
                store: if running {
                    format!("Running · {store}")
                } else {
                    store.to_owned()
                }
                .into(),
                hdr: c.hdr.into(),
                setting: if c.own {
                    setting_of(state, &g.id).label()
                } else {
                    "Default"
                }
                .into(),
                own: c.own,
                suggestion: c.suggestion.map(GameHdr::label).unwrap_or_default().into(),
                running,
                tint: tint(&g.name),
            }
        })
        .collect();
    rows.sort_by_key(|r| r.name.to_lowercase());
    ui.set_games(ModelRc::new(VecModel::from(rows)));

    let cue_count = |f: &dyn Fn(&Cues) -> bool| count(all.iter().filter(|(_, c)| f(c)).count());
    let suggest = cue_count(&|c| c.suggestion.is_some());
    let mut cue_chips = Vec::new();
    if state.config.general.game_info {
        cue_chips.push(StoreChip {
            key: "hdr".into(),
            label: "Supports HDR".into(),
            count: cue_count(&|c| matches!(c.hdr, "hdr" | "limited")),
        });
    }
    cue_chips.push(StoreChip {
        key: "own".into(),
        label: "Own setting".into(),
        count: cue_count(&|c| c.own),
    });
    if state.config.general.game_info {
        cue_chips.push(StoreChip {
            key: "suggest".into(),
            label: "Suggestions".into(),
            count: suggest,
        });
    }
    if !ui.get_cue_chips().iter().eq(cue_chips.iter().cloned()) {
        ui.set_cue_chips(ModelRc::new(VecModel::from(cue_chips)));
    }
    ui.set_suggest_count(suggest);

    let mut chips = vec![StoreChip {
        key: "all".into(),
        label: "All".into(),
        count: count(state.games.len()),
    }];
    for store in [
        Store::Steam,
        Store::Epic,
        Store::Gog,
        Store::Xbox,
        Store::Ea,
        Store::Ubisoft,
        Store::Rockstar,
        Store::BattleNet,
        Store::Riot,
        Store::HoYoPlay,
        Store::Amazon,
        Store::Itch,
        Store::Heroic,
        Store::GameBar,
        Store::Manual,
    ] {
        let n = state.games.iter().filter(|g| g.store == store).count();
        if n > 0 {
            chips.push(StoreChip {
                key: store.key().into(),
                label: store_label(store).into(),
                count: count(n),
            });
        }
    }
    // Replaced only when the counts change, so a chip keeps the keyboard focus.
    if !ui.get_chips().iter().eq(chips.iter().cloned()) {
        ui.set_chips(ModelRc::new(VecModel::from(chips)));
    }
    ui.set_filter(state.filter.as_str().into());
    ui.set_game_total(count(state.games.len()));
    show_detail(ui, state);
}

/// The first letter or digit of a name, shown where the game has no icon.
fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

fn count(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

fn show_detail(ui: &SettingsWindow, state: &State) {
    let detail = state
        .selected
        .as_ref()
        .and_then(|id| state.games.iter().find(|g| &g.id == id))
        .map(|game| detail_of(state, game));
    ui.set_selected_id(
        state
            .selected
            .as_ref()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default()
            .into(),
    );
    // The same game is updated in place, so the control being used keeps the
    // keyboard focus; another game gets a fresh panel. See `renew_detail`.
    let shown = ui.get_details();
    match (shown.row_data(0), detail) {
        (Some(old), Some(new)) if old.id == new.id => shown.set_row_data(0, new),
        (_, detail) => ui.set_details(ModelRc::new(VecModel::from(
            detail.into_iter().collect::<Vec<_>>(),
        ))),
    }
}

/// Makes the next `show_detail` build a fresh panel. Needed when a value
/// changes from outside its control (a reset, a suggestion): a control the
/// user has changed no longer follows its binding.
fn renew_detail(ui: &slint::Weak<SettingsWindow>) {
    if let Some(ui) = ui.upgrade() {
        ui.set_details(ModelRc::default());
    }
}

fn detail_of(state: &State, game: &Game) -> GameDetail {
    let settings = state.config.game(&game.id);
    let setting = settings.map(|s| s.hdr).unwrap_or_default();
    let info: Option<&GameInfo> = state.info.get(&game.id);
    let suggestion = info.and_then(|i| i.hdr.suggestion());
    let index = |hdr: GameHdr| count(HDR_SETTINGS.iter().position(|h| *h == hdr).unwrap_or(0));
    let delay = settings
        .and_then(|s| s.start_delay)
        .unwrap_or(state.config.general.start_delay);
    let hold = settings
        .and_then(|s| s.hold)
        .unwrap_or(state.config.general.hold);
    GameDetail {
        id: game.id.as_str().into(),
        name: game.name.as_str().into(),
        icon: state.icons.get(&game.id).cloned().unwrap_or_default(),
        initial: initial(&game.name).into(),
        store: game.store.label().into(),
        path: game.exe.as_deref().unwrap_or(&game.install_dir).into(),
        setting_index: index(setting),
        delay: secs(delay),
        hold: secs(hold),
        custom_timing: settings.is_some_and(|s| s.start_delay.is_some() || s.hold.is_some()),
        manual: game.store == Store::Manual,
        looked_up: info.is_some_and(|i| i.page.is_some()),
        info: match info {
            Some(i) if i.page.is_some() => format!("HDR: {}", i.hdr.label()),
            _ => info_text(state, &game.id),
        }
        .into(),
        notes: info
            .map(|i| i.hdr_notes.as_str())
            .unwrap_or_default()
            .into(),
        ray_tracing: info
            .and_then(GameInfo::ray_tracing_label)
            .unwrap_or_default()
            .into(),
        wiki_url: info.and_then(GameInfo::page_url).unwrap_or_default().into(),
        suggestion: suggestion.map(GameHdr::label).unwrap_or_default().into(),
        suggestion_index: suggestion.map_or(-1, index),
        launch_option: match (game.store, huedra_exe()) {
            (Store::Steam, Some(exe)) => launch::steam_option(&exe).into(),
            _ => Default::default(),
        },
        steam_start: state.config.steam_start(&game.id),
        shortcut: can_shortcut(game),
        launch_note: state
            .launch_note
            .as_ref()
            .filter(|(id, _)| *id == game.id)
            .map(|(_, note)| note.as_str())
            .unwrap_or_default()
            .into(),
    }
}

/// This program's path, for launch options, shortcuts and menu entries.
fn huedra_exe() -> Option<String> {
    #[cfg(feature = "sample")]
    return Some(sample::EXE.to_owned());
    #[cfg(not(feature = "sample"))]
    std::env::current_exe()
        .ok()
        .map(|exe| exe.display().to_string())
}

/// Whether a desktop shortcut can start the game directly. Steam games use
/// the launch option instead (started directly, they restart through Steam,
/// outside Huedra's reach), and Xbox games cannot be started by path.
fn can_shortcut(game: &Game) -> bool {
    game.exe.is_some() && !matches!(game.store, Store::Steam | Store::Xbox)
}

/// Writes "<game> (with HDR)" on the desktop and describes the result.
fn create_shortcut(game: &Game) -> String {
    let (Some(exe), Some(program)) = (huedra_exe(), game.exe.as_deref()) else {
        return "The shortcut could not be made: the game's program is not known.".to_owned();
    };
    let Some(desktop) = shell::desktop_folder() else {
        return "The shortcut could not be made: the desktop folder was not found.".to_owned();
    };
    let file = desktop.join(launch::shortcut_file_name(&game.name));
    let working_dir = std::path::Path::new(program)
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();
    let shortcut = shell::Shortcut {
        target: &exe,
        arguments: &launch::shortcut_arguments(program),
        working_dir: &working_dir,
        icon: program,
        description: &format!("Starts {} with HDR on", game.name),
    };
    match shell::create_shortcut(&file, &shortcut) {
        Ok(()) => format!("Shortcut created on the desktop: {}.", file.display()),
        Err(error) => format!("The shortcut could not be made: {}.", error.message()),
    }
}

fn secs(s: Seconds) -> i32 {
    i32::try_from(s.get()).unwrap_or(i32::MAX)
}

fn seconds(value: i32, max: Seconds) -> Seconds {
    Seconds::new(u32::try_from(value).unwrap_or(0)).min(max)
}

fn wire_games(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    let refresh = move |f: &dyn Fn(&mut State)| {
        if let Some(ui) = weak.upgrade() {
            with_state(|s| {
                f(s);
                show_games(&ui, s);
            });
        }
    };
    let r = refresh.clone();
    ui.on_search_changed(move |text| r(&|s| s.search = text.to_string()));
    let r = refresh.clone();
    ui.on_filter_changed(move |key| r(&|s| s.filter = key.to_string()));
    let r = refresh.clone();
    ui.on_select_game(move |id| r(&|s| s.selected = Some(GameId::from_raw(id.as_str()))));
    let r = refresh.clone();
    ui.on_close_detail(move || r(&|s| s.selected = None));

    let r = refresh.clone();
    ui.on_set_game_setting(move |id, index| {
        let hdr = HDR_SETTINGS
            .get(usize::try_from(index).unwrap_or(0))
            .copied()
            .unwrap_or_default();
        r(&|s| {
            edit(s, |c| c.game_mut(&GameId::from_raw(id.as_str())).hdr = hdr);
        });
    });
    let r = refresh.clone();
    ui.on_set_game_delay(move |id, value| {
        r(&|s| {
            edit(s, |c| {
                c.game_mut(&GameId::from_raw(id.as_str())).start_delay =
                    Some(seconds(value, huedra_core::config::MAX_DELAY));
            });
        });
    });
    let r = refresh.clone();
    ui.on_set_game_hold(move |id, value| {
        r(&|s| {
            edit(s, |c| {
                c.game_mut(&GameId::from_raw(id.as_str())).hold =
                    Some(seconds(value, huedra_core::config::MAX_HOLD));
            });
        });
    });
    let r = refresh.clone();
    let weak = ui.as_weak();
    ui.on_reset_game_timing(move |id| {
        renew_detail(&weak);
        r(&|s| {
            edit(s, |c| {
                let settings = c.game_mut(&GameId::from_raw(id.as_str()));
                settings.start_delay = None;
                settings.hold = None;
            });
        });
    });
    let r = refresh.clone();
    let weak = ui.as_weak();
    ui.on_apply_suggestion(move |id| {
        renew_detail(&weak);
        r(&|s| {
            let id = GameId::from_raw(id.as_str());
            if let Some(hdr) = s.info.get(&id).and_then(|i| i.hdr.suggestion()) {
                edit(s, |c| c.game_mut(&id).hdr = hdr);
            }
        });
    });
    let r = refresh.clone();
    let weak = ui.as_weak();
    ui.on_apply_all_suggestions(move || {
        renew_detail(&weak);
        r(&|s| {
            let changes: Vec<(GameId, GameHdr)> = s
                .games
                .iter()
                .filter_map(|g| cues(s, g).suggestion.map(|hdr| (g.id.clone(), hdr)))
                .collect();
            edit(s, |c| {
                for (id, hdr) in &changes {
                    c.game_mut(id).hdr = *hdr;
                }
            });
            if s.filter == "suggest" {
                s.filter = "all".into();
            }
        });
    });
    ui.on_open_wiki(|url| {
        shell::open_web_page(url.as_str());
    });
    let r = refresh.clone();
    let weak = ui.as_weak();
    ui.on_add_game(move || {
        let Some(path) = shell::pick_program("Choose the game's program") else {
            return;
        };
        r(&|s| {
            let id = GameId::new(Store::Manual, NormPath::new(&path).as_str());
            let name = display_name(&path)
                .trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_owned();
            edit(s, |c| {
                if c.game(&id).is_none() {
                    c.games.push(GameSettings {
                        name,
                        exe: Some(path.clone()),
                        ..GameSettings::new(id.clone())
                    });
                }
            });
            reload_games(s);
            s.selected = Some(id.clone());
        });
        if let Some(ui) = weak.upgrade() {
            start_icons(&ui);
        }
    });
    let r = refresh.clone();
    ui.on_create_shortcut(move |id| {
        r(&|s| {
            let id = GameId::from_raw(id.as_str());
            if let Some(game) = s.games.iter().find(|g| g.id == id) {
                let note = create_shortcut(game);
                s.launch_note = Some((id, note));
            }
        });
    });
    let r = refresh.clone();
    ui.on_set_game_steam_start(move |id, on| {
        r(&|s| {
            edit(s, |c| {
                // On is the default and is not written.
                c.game_mut(&GameId::from_raw(id.as_str())).steam_start = (!on).then_some(false);
            });
        });
    });
    let r = refresh.clone();
    ui.on_remove_game(move |id| {
        r(&|s| {
            edit(s, |c| c.games.retain(|g| g.id.as_str() != id.as_str()));
            reload_games(s);
            s.selected = None;
        });
    });
    let weak = ui.as_weak();
    ui.on_rescan(move || {
        if let Some(ui) = weak.upgrade() {
            with_state(|s| {
                reload_games(s);
                ui.set_scan_note(format!("{} games found.", s.games.len()).into());
                show_games(&ui, s);
            });
            start_icons(&ui);
            let _ = tray(&Request::Reload);
        }
    });
}

fn reload_games(state: &mut State) {
    state.games = scan_games(&state.config);
}

/// Finds icons for the games that have none yet; icons kept from earlier
/// runs arrive first. Discord's list is used only while online lookups are on.
fn start_icons(ui: &SettingsWindow) {
    if cfg!(feature = "sample") {
        return;
    }
    let Some((games, folder, online)) = with_state(|s| {
        let games: Vec<Game> = s
            .games
            .iter()
            .filter(|g| !s.icons.contains_key(&g.id))
            .cloned()
            .collect();
        (games, s.locations.icons.clone(), s.config.general.game_info)
    }) else {
        return;
    };
    if games.is_empty() {
        return;
    }
    let weak = ui.as_weak();
    icons::spawn(games, folder, online, move |icon| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                let image = rgba_image(&icon.rgba, icon.width, icon.height);
                s.icons.insert(icon.id, image);
                show_games(&ui, s);
            });
        });
    });
}

fn start_lookups(ui: &SettingsWindow) {
    if cfg!(feature = "sample") {
        return;
    }
    let Some((items, path)) = with_state(|s| {
        if !s.config.general.game_info {
            return None;
        }
        let items = s
            .games
            .iter()
            .filter(|g| g.store != Store::Manual)
            .map(|g| Lookup {
                id: g.id.clone(),
                name: g.name.clone(),
            })
            .collect();
        Some((items, s.locations.game_info.clone()))
    })
    .flatten() else {
        return;
    };
    let weak = ui.as_weak();
    lookups::spawn(items, path, move |info| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                s.info.put(info);
                show_games(&ui, s);
            });
        });
    });
}

// ---- Displays ---------------------------------------------------------

fn show_displays(ui: &SettingsWindow, state: &State) {
    let rows: Vec<DisplayRow> = state
        .displays
        .iter()
        .map(|d| {
            let capable = d.support.is_capable();
            let controlled = capable && state.config.is_managed(d.id.as_str());
            let mut parts = Vec::new();
            if d.primary {
                parts.push("Main display".to_owned());
            }
            match d.support {
                HdrSupport::Supported => {}
                HdrSupport::LimitedByPolicy => {
                    parts.push("HDR limited by Windows policy".to_owned());
                }
                HdrSupport::Unsupported => {}
            }
            if let Some(peak) = d.peak.filter(|_| capable) {
                parts.push(format!("Peak brightness {peak}, as the display reports it"));
            }
            let mut detail = parts.join(" · ");
            if !detail.is_empty() {
                detail.push_str(". ");
            }
            detail.push_str(if !capable {
                "No HDR support, so Huedra cannot change it."
            } else if controlled {
                "Huedra turns HDR on and off here."
            } else {
                "Huedra does not change this display. HDR stays as you set it in Windows."
            });
            let control = if !capable {
                "No HDR support"
            } else if controlled {
                "Controlled by Huedra"
            } else {
                "Not controlled by Huedra"
            };
            DisplayRow {
                id: d.id.as_str().into(),
                number: i32::try_from(d.number).unwrap_or(0),
                title: if d.name.trim().is_empty() {
                    format!("Display {}", d.number)
                } else {
                    format!("Display {} · {}", d.number, d.name)
                }
                .into(),
                detail: detail.into(),
                capable,
                controlled,
                hdr_on: d.state == HdrState::On,
                status: if d.primary {
                    // "Main display · controlled by Huedra": only the
                    // first word gives up its capital.
                    format!(
                        "Main display · {}{}",
                        control[..1].to_lowercase(),
                        &control[1..]
                    )
                } else {
                    control.to_owned()
                }
                .into(),
            }
        })
        .collect();
    ui.set_status_displays(ModelRc::new(VecModel::from(rows.clone())));
    ui.set_displays(ModelRc::new(VecModel::from(rows)));
    ui.set_retries(i32::try_from(state.config.general.retry_attempts).unwrap_or(0));
    ui.set_content_only(state.config.displays.content_only);
    ui.set_main_only(state.config.displays.manual == ManualDisplays::Main);
}

fn wire_displays(ui: &SettingsWindow) {
    ui.on_set_content_only(|on| {
        with_state(|s| edit(s, |c| c.displays.content_only = on));
    });
    ui.on_set_main_only(|on| {
        let manual = if on {
            ManualDisplays::Main
        } else {
            ManualDisplays::All
        };
        with_state(|s| edit(s, |c| c.displays.manual = manual));
    });
    let weak = ui.as_weak();
    ui.on_set_controlled(move |id, controlled| {
        with_state(|s| {
            let id = id.to_string();
            edit(s, |c| {
                c.displays
                    .unmanaged
                    .retain(|d| !d.eq_ignore_ascii_case(&id));
                if !controlled {
                    c.displays.unmanaged.push(id);
                }
            });
            if let Some(ui) = weak.upgrade() {
                show_displays(&ui, s);
            }
        });
    });
    ui.on_set_retries(|n| {
        with_state(|s| {
            edit(s, |c| {
                c.general.retry_attempts = u32::try_from(n)
                    .unwrap_or(0)
                    .min(huedra_core::config::MAX_RETRIES);
            });
        });
    });
}

// ---- Video players ----------------------------------------------------

fn video_row(state: &State, app: VideoApp) -> VideoAppRow {
    let choices = app.choices();
    let mode = state.config.video.mode(app);
    let labels: Vec<slint::SharedString> = choices.iter().map(|c| c.label().into()).collect();
    VideoAppRow {
        key: app.key().into(),
        name: app.label().into(),
        choices: ModelRc::new(VecModel::from(labels)),
        // A browser set to While open behaves as When HDR video plays.
        index: choices
            .iter()
            .position(|c| *c == mode)
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(0),
        active: state
            .running
            .iter()
            .any(|id| video::app_of(&TriggerId::new(id.as_str())) == Some(app)),
    }
}

fn show_video(ui: &SettingsWindow, state: &State) {
    let rows = |browsers: bool| -> ModelRc<VideoAppRow> {
        let rows: Vec<VideoAppRow> = VideoApp::ALL
            .into_iter()
            .filter(|app| app.is_browser() == browsers)
            .map(|app| video_row(state, app))
            .collect();
        ModelRc::new(VecModel::from(rows))
    };
    ui.set_video_players(rows(false));
    ui.set_video_browsers(rows(true));
    let v = &state.config.video;
    ui.set_video_keywords(video::keywords_text(&v.keywords).into());
    ui.set_video_full_screen_only(v.full_screen_only);
    ui.set_video_sdr_with_rtx(v.sdr_with_rtx);
    ui.set_video_rtx_note(rtx_note(registry::rtx_video_hdr()).into());
    let sites: Vec<SiteRow> = StreamingSite::ALL
        .into_iter()
        .map(|site| SiteRow {
            key: site.key().into(),
            name: site.label().into(),
            address: site.address().into(),
            on: v.sites.contains(&site),
        })
        .collect();
    ui.set_video_sites(ModelRc::new(VecModel::from(sites)));
    ui.set_video_hold(v.hold.map_or(0, secs));
    ui.set_video_hold_custom(v.hold.is_some());
}

/// The description of the RTX Video HDR switch, from the NVIDIA setting
/// (`None` without an NVIDIA graphics card).
fn rtx_note(rtx: Option<bool>) -> &'static str {
    match rtx {
        Some(true) => {
            "RTX Video HDR is on in the NVIDIA settings. Full-screen video with sound in a player or browser turns HDR on, so the driver can convert it."
        }
        Some(false) => {
            "RTX Video HDR is off in the NVIDIA settings, so this has no effect now. When it is on, full-screen video with sound turns HDR on, so the driver can convert it."
        }
        None => "Needs an NVIDIA RTX graphics card; none was found.",
    }
}

fn wire_video(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    let edit_video = move |change: &dyn Fn(&mut Config)| {
        with_state(|s| {
            edit(s, change);
            if let Some(ui) = weak.upgrade() {
                show_video(&ui, s);
            }
        });
    };
    let edit_video = std::rc::Rc::new(edit_video);
    let e = edit_video.clone();
    ui.on_set_video_app(move |key, index| {
        let Some(app) = VideoApp::ALL.into_iter().find(|a| a.key() == key.as_str()) else {
            return;
        };
        let Some(mode) = usize::try_from(index)
            .ok()
            .and_then(|i| app.choices().get(i).copied())
        else {
            return;
        };
        e(&|c| c.video.set_mode(app, mode));
    });
    let e = edit_video.clone();
    ui.on_set_video_keywords(move |text| {
        e(&|c| c.video.set_keywords_text(&text));
    });
    let e = edit_video.clone();
    ui.on_set_video_full_screen_only(move |on| {
        e(&|c| c.video.full_screen_only = on);
    });
    let e = edit_video.clone();
    ui.on_set_video_sdr_with_rtx(move |on| {
        e(&|c| c.video.sdr_with_rtx = on);
    });
    let e = edit_video.clone();
    ui.on_set_video_site(move |key, on| {
        if let Some(site) = StreamingSite::from_key(&key) {
            e(&|c| c.video.set_site(site, on));
        }
    });
    let e = edit_video.clone();
    ui.on_set_video_hold(move |v| {
        e(&|c| c.video.hold = Some(seconds(v, huedra_core::config::MAX_HOLD)));
    });
    ui.on_reset_video_hold(move || {
        edit_video(&|c| c.video.hold = None);
    });
}

// ---- Settings ---------------------------------------------------------

/// Whether the start-up entry starts this program. An entry for another
/// copy of Huedra (an installed one, seen from a portable copy) does not
/// count, so turning the switch off never removes that copy's entry.
fn starts_with_windows() -> bool {
    registry::startup_command().is_some_and(|entry| {
        std::env::current_exe().is_ok_and(|exe| command::startup_starts(&entry, &exe))
    })
}

fn show_settings(ui: &SettingsWindow, state: &State) {
    let g = &state.config.general;
    ui.set_start_with_windows(starts_with_windows());
    ui.set_context_menu(registry::context_menu_command().is_some());
    ui.set_updates(match g.updates {
        UpdatePolicy::Install => 0,
        UpdatePolicy::Ask => 1,
        UpdatePolicy::Off => 2,
    });
    ui.set_close_action(match g.on_close {
        CloseAction::KeepRunning => 0,
        CloseAction::Quit => 1,
    });
    ui.set_game_info(g.game_info);
    ui.set_hold(secs(g.hold));
    ui.set_delay(secs(g.start_delay));
    ui.set_hotkeys(g.hotkeys);
    ui.set_log_titles(g.log_window_titles);
}

fn wire_settings(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    ui.on_set_start_with_windows(move |on| {
        let command = std::env::current_exe()
            .ok()
            .map(|exe| command::startup_command(&exe));
        let result = if cfg!(feature = "sample") {
            Ok(())
        } else {
            registry::set_startup(if on { command.as_deref() } else { None })
        };
        with_state(|s| {
            edit(s, |c| c.general.start_with_windows = on && result.is_ok());
        });
        if let Some(ui) = weak.upgrade() {
            ui.set_start_with_windows(starts_with_windows());
        }
    });
    let weak = ui.as_weak();
    ui.on_set_context_menu(move |on| {
        let exe = huedra_exe();
        let command = exe.as_deref().map(launch::context_menu_command);
        let entry = match (&exe, &command) {
            (Some(exe), Some(command)) if on => Some(registry::ContextMenu {
                text: "Start with HDR",
                icon: exe,
                command,
            }),
            _ => None,
        };
        if !cfg!(feature = "sample") {
            let _ = registry::set_context_menu(entry);
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_context_menu(registry::context_menu_command().is_some());
        }
    });
    ui.on_set_close_action(|index| {
        with_state(|s| {
            edit(s, |c| {
                c.general.on_close = match index {
                    1 => CloseAction::Quit,
                    _ => CloseAction::KeepRunning,
                };
            });
        });
    });
    ui.on_set_updates(|index| {
        with_state(|s| {
            edit(s, |c| {
                c.general.updates = match index {
                    1 => UpdatePolicy::Ask,
                    2 => UpdatePolicy::Off,
                    _ => UpdatePolicy::Install,
                };
            });
        });
    });
    // These change what the expanders sum up in their headers (and the
    // shortcut labels on Status), so the page is shown again after each.
    let weak = ui.as_weak();
    let change = std::rc::Rc::new(move |change: &dyn Fn(&mut Config)| {
        with_state(|s| {
            edit(s, change);
            if let Some(ui) = weak.upgrade() {
                show_settings(&ui, s);
            }
        });
    });
    let weak = ui.as_weak();
    let c = change.clone();
    ui.on_set_game_info(move |on| {
        c(&|c| c.general.game_info = on);
        if let Some(ui) = weak.upgrade() {
            with_state(|s| show_games(&ui, s));
            if on {
                start_lookups(&ui);
                start_icons(&ui);
            }
        }
    });
    let c = change.clone();
    ui.on_set_hold(move |v| {
        c(&|c| c.general.hold = seconds(v, huedra_core::config::MAX_HOLD));
    });
    let c = change.clone();
    ui.on_set_delay(move |v| {
        c(&|c| c.general.start_delay = seconds(v, huedra_core::config::MAX_DELAY));
    });
    let c = change.clone();
    ui.on_set_hotkeys(move |on| {
        c(&|c| c.general.hotkeys = on);
    });
    ui.on_set_log_titles(move |on| {
        change(&|c| c.general.log_window_titles = on);
    });
}

// ---- Window geometry --------------------------------------------------

fn restore_geometry(ui: &SettingsWindow) {
    let Some(geometry) = with_state(|s| WindowGeometry::load(&s.locations.window)).flatten() else {
        return;
    };
    let window = ui.window();
    window.set_size(PhysicalSize::new(geometry.width, geometry.height));
    if instance::on_screen(geometry.x + 40, geometry.y + 20) {
        window.set_position(PhysicalPosition::new(geometry.x, geometry.y));
    }
    if geometry.maximized {
        window.set_maximized(true);
    }
}

fn save_geometry(ui: &SettingsWindow) {
    let window = ui.window();
    let size = window.size();
    let position = window.position();
    let geometry = WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized(),
    };
    with_state(|s| {
        let _ = geometry.save(&s.locations.window);
    });
}

// ---- Updates ----------------------------------------------------------

/// The newer version waiting to be installed, or an empty string.
fn install_version(path: &std::path::Path) -> String {
    use huedra_core::update::{UpdateState, Version};
    UpdateState::load(path)
        .waiting(Version::current())
        .map(|v| v.to_string())
        .unwrap_or_default()
}

/// The version of the program file now in place, from `huedra --version`
/// ("Huedra 1.2.3"); after an update it is newer than this process.
fn program_version() -> Option<huedra_core::update::Version> {
    use std::os::windows::process::CommandExt as _;
    /// `CREATE_NO_WINDOW`: no console window for the child process.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().ok()?;
    let output = std::process::Command::new(exe)
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    text.split_whitespace().nth(1)?.parse().ok()
}

/// Runs this program with `args` and no console window, then calls `done`
/// on the window's thread with whether it succeeded. The program logs its
/// own failures; a program that did not start at all is logged here.
fn run_program(
    ui: &SettingsWindow,
    args: &'static [&'static str],
    done: impl FnOnce(&SettingsWindow, bool) + Send + 'static,
) {
    use std::os::windows::process::CommandExt as _;
    use std::process::Stdio;
    /// `CREATE_NO_WINDOW`: no console window for the child process.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let weak = ui.as_weak();
    let log = with_state(|s| s.locations.log.clone());
    thread::spawn(move || {
        let status = std::env::current_exe().and_then(|exe| {
            std::process::Command::new(exe)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status()
        });
        if let (Err(error), Some(path)) = (&status, log) {
            let log = huedra_core::logfile::LogFile::new(path, huedra_core::logfile::MAX_BYTES);
            let _ = log.append(&format!(
                "{} Could not start `huedra {}`: {error}",
                huedra_win::clock::now(),
                args.join(" ")
            ));
        }
        let ok = status.is_ok_and(|s| s.success());
        let _ = weak.upgrade_in_event_loop(move |ui| done(&ui, ok));
    });
}

/// Check for updates, install, the project's links and the disclaimer.
fn wire_about(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    ui.on_check_updates(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        ui.set_checking_updates(true);
        ui.set_update_check_status("Checking for updates...".into());
        run_program(&ui, &["update", "--check"], |ui, ok| {
            ui.set_checking_updates(false);
            let Some(path) = with_state(|s| s.locations.update.clone()) else {
                return;
            };
            let waiting = install_version(&path);
            ui.set_update_check_status(
                if !ok {
                    "The check did not succeed; the log has the reason.".to_owned()
                } else if waiting.is_empty() {
                    format!("Huedra {} is up to date.", huedra_core::VERSION)
                } else {
                    format!("Huedra {waiting} is available.")
                }
                .into(),
            );
            let mut note = update_note(&path);
            if !ok {
                note = format!("The check did not succeed; the log has the reason. {note}");
            }
            ui.set_update_note(note.into());
            ui.set_install_version(install_version(&path).into());
            offer_update(ui, true);
        });
    });
    let weak = ui.as_weak();
    ui.on_install_update(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        ui.set_installing_update(true);
        ui.set_update_status(
            "Downloading and installing. Huedra starts again in the notification area when it is done."
                .into(),
        );
        run_program(&ui, &["update"], |ui, ok| {
            use huedra_core::update::Version;
            ui.set_installing_update(false);
            ui.set_install_version("".into());
            // `huedra update` also succeeds when there was nothing newer to
            // install; the program now in place tells which it was.
            let installed = program_version().is_some_and(|v| v > Version::current());
            if ok && installed {
                ui.set_update_status("Installed. This window opens again with the new version.".into());
                REOPEN.set(true);
                let _ = slint::quit_event_loop();
            } else if ok {
                ui.set_update_status("Huedra is up to date; there was nothing to install.".into());
            } else {
                ui.set_update_status(
                    "The update was not installed. If a game or video is using HDR, try again after it ends; otherwise the log has the reason."
                        .into(),
                );
            }
        });
    });
    ui.on_open_changelog(|| {
        if let Some(url) = huedra_core::update::changelog_page() {
            shell::open_web_page(&url);
        }
    });
    ui.on_open_project(|| {
        if let Some(url) = huedra_core::update::project_page() {
            shell::open_web_page(&url);
        }
    });
    ui.on_open_support(|| {
        shell::open_web_page(huedra_core::update::SUPPORT_URL);
    });
    ui.on_open_extension(|| {
        shell::open_web_page(huedra_core::video::EXTENSION_STORE_URL);
    });
    let weak = ui.as_weak();
    ui.on_accept_disclaimer(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        ui.set_show_disclaimer(false);
        if ui.get_disclaimer_accepted() {
            return;
        }
        ui.set_disclaimer_accepted(true);
        let setup = with_state(|s| {
            edit(s, |c| c.general.disclaimer_accepted = true);
            !s.locations.portable && s.config.general.start_with_windows
        });
        // What the tray did on its first start before there was a
        // disclaimer: Start with Windows is on by default, so a copy that is
        // not portable adds the start-up entry.
        if setup == Some(true)
            && registry::startup_command().is_none()
            && let Ok(exe) = std::env::current_exe()
        {
            if !cfg!(feature = "sample") {
                let _ = registry::set_startup(Some(&command::startup_command(&exe)));
            }
            ui.set_start_with_windows(starts_with_windows());
        }
        if !cfg!(feature = "sample")
            && !matches!(tray(&Request::Status), Ok(Response::Status(_)))
            && let Ok(exe) = std::env::current_exe()
        {
            // The window is open already; the tray only joins it.
            huedra_win::process::spawn_detached(&exe, &[command::BACKGROUND]);
        }
    });
    ui.on_decline_disclaimer(|| {
        let _ = slint::quit_event_loop();
    });
}

/// Shows the update dialog for the version waiting, with its notes when a
/// check kept them: once per version in this window, or again when `force`
/// (after Check for updates). Not while the disclaimer is open.
fn offer_update(ui: &SettingsWindow, force: bool) {
    use huedra_core::update::{self, NoteLine, ReleaseNotes, UpdateState, Version};

    if ui.get_show_disclaimer() {
        return;
    }
    let Some(path) = with_state(|s| s.locations.update.clone()) else {
        return;
    };
    let Some(version) = UpdateState::load(&path).waiting(Version::current()) else {
        return;
    };
    if !force && with_state(|s| s.update_shown).flatten() == Some(version) {
        return;
    }
    with_state(|s| s.update_shown = Some(version));
    let max = u64::try_from(update::MAX_NOTES_BYTES).unwrap_or(u64::MAX);
    let notes = huedra_core::files::read_limited(&update::notes_path(&path), max)
        .ok()
        .flatten()
        .and_then(|text| ReleaseNotes::parse(&text, version));
    let (lines, more) = notes
        .map(|n| n.summary(UPDATE_DIALOG_ITEMS))
        .unwrap_or_default();
    let rows: Vec<UpdateNote> = lines
        .into_iter()
        .map(|line| match line {
            NoteLine::Heading(text) => UpdateNote {
                heading: true,
                text: text.into(),
            },
            NoteLine::Item(text) => UpdateNote {
                heading: false,
                text: text.into(),
            },
        })
        .collect();
    ui.set_update_version(version.to_string().into());
    ui.set_update_notes(ModelRc::new(VecModel::from(rows)));
    ui.set_update_more(i32::try_from(more).unwrap_or(i32::MAX));
    ui.set_has_changelog(update::changelog_page().is_some());
    ui.set_update_status("".into());
    ui.set_show_update(true);
}

/// The Updates row's extra sentence: whether this build checks, and when
/// it last did. A version waiting shows in its own notice.
fn update_note(path: &std::path::Path) -> String {
    use huedra_core::update::{self, UpdateState};

    if !update::is_available() {
        return "This build has no release address, so no checks are made.".to_owned();
    }
    let state = UpdateState::load(path);
    if state.last_check == 0 {
        return "Not checked yet.".to_owned();
    }
    let now = update::unix_secs(std::time::SystemTime::now());
    match now.saturating_sub(state.last_check) / 3600 {
        0 => "Last checked less than an hour ago.".to_owned(),
        1 => "Last checked an hour ago.".to_owned(),
        hours => format!("Last checked {hours} hours ago."),
    }
}
