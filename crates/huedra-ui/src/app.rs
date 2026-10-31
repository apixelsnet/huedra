//! The settings window: loads the settings, games and displays, keeps the
//! Status page in step with the tray process, and saves every change at once
//! (then asks the tray process to reload). Feature doc:
//! `docs/features/08-settings-window.md`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use huedra_core::config::{Config, GameHdr, GameSettings, Seconds, UpdatePolicy};
use huedra_core::display::{Display, HdrState, HdrSupport};
use huedra_core::engine::{Mode, TriggerId};
use huedra_core::gameinfo::{Cache, GameInfo};
use huedra_core::geometry::WindowGeometry;
use huedra_core::icon::{self, TrayIcon};
use huedra_core::ipc::{HistoryKind, Request, Response, StatusReport};
use huedra_core::launch;
use huedra_core::library::{Game, GameId, Store};
use huedra_core::locations::Locations;
use huedra_core::paths::{NormPath, display_name};
use huedra_core::video::{self, StreamingSite, VideoApp};
use huedra_win::{display, instance, pipe, registry, shell, stores};
use slint::{
    CloseRequestResponse, ComponentHandle, Image, Model, ModelRc, PhysicalPosition, PhysicalSize,
    Rgba8Pixel, SharedPixelBuffer, VecModel,
};

use crate::icons;
use crate::lookups::{self, Lookup};
use crate::ui::{
    DisplayRow, GameDetail, GameRow, HistoryRow, SettingsWindow, SiteRow, StoreChip, Theme,
    VideoAppRow,
};

/// The window's title; also used to find an open window.
const TITLE: &str = "Huedra";
/// How often the Status page asks the tray process for its state.
const STATUS_POLL: Duration = Duration::from_secs(1);
const ICON_SIZE: u32 = 64;
const DOT_SIZE: u32 = 36;
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
    /// General.
    General = 4,
}

impl Page {
    /// Parses a page name as given on the command line.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "status" => Some(Self::Status),
            "games" => Some(Self::Games),
            "video" => Some(Self::Video),
            "displays" => Some(Self::Displays),
            "general" => Some(Self::General),
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
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with_borrow_mut(|state| state.as_mut().map(f))
}

/// Opens the settings window and runs it until it is closed. If a settings
/// window is already open, brings that one to the front instead.
///
/// # Errors
/// Slint platform errors (no graphics or window system).
pub fn run(locations: Locations, page: Page) -> Result<(), slint::PlatformError> {
    if instance::activate_window(TITLE) {
        return Ok(());
    }
    // The software renderer needs a tenth of the memory of OpenGL and starts
    // faster; this window has nothing it cannot draw.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let ui = SettingsWindow::new()?;
    let theme = ui.global::<Theme>();
    theme.set_motion(shell::animations_enabled());
    theme.set_spectrum_image(rgba_image(
        &icon::spectrum_strip(SPECTRUM_W, SPECTRUM_H),
        SPECTRUM_W,
        SPECTRUM_H,
    ));
    theme.set_dot_image(rgba_image(
        &icon::render(TrayIcon::Hdr, DOT_SIZE),
        DOT_SIZE,
        DOT_SIZE,
    ));
    ui.set_light_theme(registry::apps_use_light_theme());
    let weak = ui.as_weak();
    registry::watch_light_theme(move |light| {
        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_light_theme(light));
    });
    ui.set_page(page as i32);
    ui.set_app_icon(app_icon());
    ui.set_version(huedra_core::VERSION.into());
    ui.set_data_folder(
        locations
            .config
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
            .into(),
    );

    let config = Config::load(&locations.config).unwrap_or_default();
    let mut games = config.manual_games();
    games.extend(stores::scan());
    let state = State {
        info: Cache::load(&locations.game_info),
        displays: display::displays().unwrap_or_default(),
        games,
        config,
        locations,
        running: Vec::new(),
        icons: HashMap::new(),
        search: String::new(),
        filter: "all".into(),
        selected: None,
        launch_note: None,
    };
    STATE.set(Some(state));

    with_state(|s| {
        show_games(&ui, s);
        show_video(&ui, s);
        show_displays(&ui, s);
        show_general(&ui, s);
        show_status(&ui, s, None);
    });
    wire_status(&ui);
    wire_games(&ui);
    wire_video(&ui);
    wire_displays(&ui);
    wire_general(&ui);
    restore_geometry(&ui);
    start_status_polling(&ui);
    start_lookups(&ui);
    start_icons(&ui);

    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            save_geometry(&ui);
        }
        CloseRequestResponse::HideWindow
    });
    limit_size_once_shown(0);
    ui.run()
}

/// Slint's software renderer works in 16-bit pixel coordinates and stops
/// at a larger window; this stays well below that.
const MAX_WINDOW_PIXELS: u16 = 16_384;

/// Limits the window's size once the event loop has created it.
fn limit_size_once_shown(attempt: u32) {
    slint::Timer::single_shot(Duration::from_millis(50), move || {
        if instance::limit_window_size(MAX_WINDOW_PIXELS) == 0 && attempt < 40 {
            limit_size_once_shown(attempt + 1);
        }
    });
}

fn app_icon() -> Image {
    rgba_image(
        &icon::render(TrayIcon::Hdr, ICON_SIZE),
        ICON_SIZE,
        ICON_SIZE,
    )
}

fn rgba_image(rgba: &[u8], width: u32, height: u32) -> Image {
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
            let _ = pipe::call(&Request::Reload);
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
    ui.on_start_huedra(|| {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
    });
}

/// Sends a request without blocking the window; the next status poll shows
/// the result.
fn send(request: Request) {
    thread::spawn(move || {
        let _ = pipe::call(&request);
    });
}

fn start_status_polling(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    let _ = thread::Builder::new()
        .name("huedra-status".into())
        .spawn(move || {
            loop {
                let report = match pipe::call(&Request::Status) {
                    Ok(Response::Status(report)) => Some(report),
                    _ => None,
                };
                let delivered = weak.upgrade_in_event_loop(move |ui| {
                    with_state(|s| show_status(&ui, s, report.as_ref()));
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
        ui.set_detail("Automatic switching is off while Huedra is not running.".into());
        ui.set_caption(caption(state, on).into());
        ui.set_history(ModelRc::default());
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
    let history: Vec<HistoryRow> = report
        .history
        .iter()
        .map(|h| HistoryRow {
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
    set_running(ui, state, report.active_games.clone());
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
        return "sRGB colour · standard brightness".to_owned();
    }
    let peak = state
        .displays
        .iter()
        .filter(|d| d.state == HdrState::On)
        .filter_map(|d| d.peak)
        .map(|n| n.get())
        .fold(0.0_f32, f32::max);
    if peak > 0.0 {
        format!("BT.2020 colour · up to {peak:.0} nits")
    } else {
        "BT.2020 colour".to_owned()
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
    let mut rows: Vec<GameRow> = state
        .games
        .iter()
        .filter(|g| state.filter == "all" || g.store.key() == state.filter)
        .filter(|g| search.is_empty() || g.name.to_lowercase().contains(&search))
        .map(|g| {
            let running = state.running.iter().any(|r| r == g.id.as_str());
            let info = info_text(state, &g.id);
            GameRow {
                id: g.id.as_str().into(),
                name: g.name.as_str().into(),
                icon: state.icons.get(&g.id).cloned().unwrap_or_default(),
                initial: initial(&g.name).into(),
                store: (if g.store == Store::Manual {
                    "Added"
                } else {
                    g.store.label()
                })
                .into(),
                detail: if running {
                    format!("Running · {info}")
                } else {
                    info
                }
                .into(),
                setting: setting_of(state, &g.id).label().into(),
                running,
            }
        })
        .collect();
    rows.sort_by_key(|r| r.name.to_lowercase());
    ui.set_games(ModelRc::new(VecModel::from(rows)));

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
        Store::Manual,
    ] {
        let n = state.games.iter().filter(|g| g.store == store).count();
        if n > 0 {
            chips.push(StoreChip {
                key: store.key().into(),
                label: (if store == Store::Manual {
                    "Added"
                } else {
                    store.label()
                })
                .into(),
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
            let _ = pipe::call(&Request::Reload);
        }
    });
}

fn reload_games(state: &mut State) {
    let mut games = state.config.manual_games();
    games.extend(stores::scan());
    state.games = games;
}

/// Finds icons for the games that have none yet; icons kept from earlier
/// runs arrive first. Discord's list is used only while online lookups are on.
fn start_icons(ui: &SettingsWindow) {
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
            let mut parts = vec![match d.support {
                HdrSupport::Supported => "Supports HDR".to_owned(),
                HdrSupport::LimitedByPolicy => "HDR limited by Windows policy".to_owned(),
                HdrSupport::Unsupported => "Does not support HDR".to_owned(),
            }];
            if let Some(peak) = d.peak.filter(|_| d.support.is_capable()) {
                parts.push(format!("peak brightness {peak} (reported by the display)"));
            }
            if d.primary {
                parts.push("main display".to_owned());
            }
            DisplayRow {
                id: d.id.as_str().into(),
                number: i32::try_from(d.number).unwrap_or(0),
                title: if d.name.trim().is_empty() {
                    format!("Display {}", d.number)
                } else {
                    format!("Display {} · {}", d.number, d.name)
                }
                .into(),
                detail: parts.join(" · ").into(),
                capable: d.support.is_capable(),
                managed: d.support.is_capable() && state.config.is_managed(d.id.as_str()),
                hdr_on: d.state == HdrState::On,
            }
        })
        .collect();
    ui.set_displays(ModelRc::new(VecModel::from(rows)));
    ui.set_retries(i32::try_from(state.config.general.retry_attempts).unwrap_or(0));
    ui.set_content_only(state.config.displays.content_only);
}

fn wire_displays(ui: &SettingsWindow) {
    ui.on_set_content_only(|on| {
        with_state(|s| edit(s, |c| c.displays.content_only = on));
    });
    ui.on_set_managed(|id, managed| {
        with_state(|s| {
            let id = id.to_string();
            edit(s, |c| {
                c.displays
                    .unmanaged
                    .retain(|d| !d.eq_ignore_ascii_case(&id));
                if !managed {
                    c.displays.unmanaged.push(id);
                }
            });
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

// ---- General ----------------------------------------------------------

fn show_general(ui: &SettingsWindow, state: &State) {
    let g = &state.config.general;
    ui.set_start_with_windows(registry::startup_command().is_some());
    ui.set_context_menu(registry::context_menu_command().is_some());
    ui.set_updates(match g.updates {
        UpdatePolicy::Install => 0,
        UpdatePolicy::Ask => 1,
        UpdatePolicy::Off => 2,
    });
    ui.set_game_info(g.game_info);
    ui.set_hold(secs(g.hold));
    ui.set_delay(secs(g.start_delay));
    ui.set_hotkeys(g.hotkeys);
    ui.set_log_titles(g.log_window_titles);
}

fn wire_general(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    ui.on_set_start_with_windows(move |on| {
        let command = std::env::current_exe()
            .ok()
            .map(|exe| format!("\"{}\"", exe.display()));
        let result = registry::set_startup(if on { command.as_deref() } else { None });
        with_state(|s| {
            edit(s, |c| c.general.start_with_windows = on && result.is_ok());
        });
        if let Some(ui) = weak.upgrade() {
            ui.set_start_with_windows(registry::startup_command().is_some());
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
        let _ = registry::set_context_menu(entry);
        if let Some(ui) = weak.upgrade() {
            ui.set_context_menu(registry::context_menu_command().is_some());
        }
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
    let weak = ui.as_weak();
    ui.on_set_game_info(move |on| {
        with_state(|s| {
            edit(s, |c| c.general.game_info = on);
        });
        if let Some(ui) = weak.upgrade() {
            with_state(|s| show_games(&ui, s));
            if on {
                start_lookups(&ui);
                start_icons(&ui);
            }
        }
    });
    ui.on_set_hold(|v| {
        with_state(|s| {
            edit(s, |c| {
                c.general.hold = seconds(v, huedra_core::config::MAX_HOLD)
            });
        });
    });
    ui.on_set_delay(|v| {
        with_state(|s| {
            edit(s, |c| {
                c.general.start_delay = seconds(v, huedra_core::config::MAX_DELAY)
            });
        });
    });
    ui.on_set_hotkeys(|on| {
        with_state(|s| {
            edit(s, |c| c.general.hotkeys = on);
        });
    });
    ui.on_set_log_titles(|on| {
        with_state(|s| {
            edit(s, |c| c.general.log_window_titles = on);
        });
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
