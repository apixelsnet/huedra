//! The settings window: loads the settings, games and displays, keeps the
//! Status page in step with the tray process, and saves every change at once
//! (then asks the tray process to reload). Feature doc:
//! `docs/features/08-settings-window.md`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use huedra_core::color::Installed;
use huedra_core::command;
use huedra_core::config::{
    CloseAction, Config, GameHdr, GameSettings, ManualDisplays, Seconds, UpdatePolicy,
};
use huedra_core::display::{Display, DisplayId, HdrPeaks, HdrState, HdrSupport, SdrBrightness};
use huedra_core::engine::{Mode, TriggerId};
use huedra_core::gameinfo::{Cache, GameInfo};
use huedra_core::geometry::WindowGeometry;
use huedra_core::icon;
use huedra_core::ipc::{HistoryKind, Request, Response, StatusReport};
use huedra_core::launch;
use huedra_core::library::{self, Game, GameId, Store};
use huedra_core::locations::Locations;
use huedra_core::mode::DisplayMode;
use huedra_core::mods::{self as hdr_mods, Manifest, Mod};
use huedra_core::paths::{NormPath, display_name};
use huedra_core::shortcut::Binding;
use huedra_core::video::{self, StreamingSite, VideoApp};
use huedra_win::hdr_layer::{HdrLayer, PixelRect, Placement};
use huedra_win::{instance, pipe, registry, shell};
use slint::winit_030::{EventResult, WinitWindowAccessor as _, winit};
use slint::{
    CloseRequestResponse, ComponentHandle, Image, Model, ModelRc, PhysicalPosition, PhysicalSize,
    Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};

use crate::icons;
use crate::lookups::{self, Lookup};
use crate::mods::{self as mod_jobs, Plan};
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
/// How many status rounds pass between readings of the displays' color
/// profiles.
const CALIBRATION_ROUNDS: u32 = 10;
/// The Windows HDR Calibration app in the Microsoft Store.
const CALIBRATION_APP: &str = "https://apps.microsoft.com/detail/9n7f2sm5d1lr";
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
    /// HDR displays without an HDR color profile, offered calibration.
    uncalibrated: Vec<DisplayId>,
    /// The peak each display reported while in HDR (`peaks.toml`).
    hdr_peaks: HdrPeaks,
    /// The installed color profiles a game can choose.
    profiles: Installed,
    /// The modes the main display offers, for a game's mode.
    modes: Vec<DisplayMode>,
    running: Vec<String>,
    icons: HashMap<GameId, Image>,
    /// Portrait art for the cover grid, loaded once the grid is shown.
    covers: HashMap<GameId, Image>,
    covers_started: bool,
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
    /// The RenoDX and Luma lists.
    mods: Vec<Mod>,
    /// The records of the mods Huedra installed.
    installed: Vec<Manifest>,
    /// An NVIDIA driver answers, so RTX HDR can be set.
    rtx_available: bool,
    /// The game the HDR mod notice was shown for, installed once accepted.
    mod_pending: Option<GameId>,
    /// The install waiting for the user's confirmation.
    mod_plan: Option<Plan>,
    /// The removal waiting for the user's confirmation.
    mod_remove: Option<Manifest>,
    /// The removal of a mod installed by hand waiting for the user's
    /// confirmation: the game, its name, the project and what goes.
    mod_remove_hand: Option<(GameId, String, hdr_mods::Source, hdr_mods::HandRemoval)>,
    /// The game whose mod is being installed or removed.
    mod_busy: Option<GameId>,
    /// Set to stop the install under way.
    mod_cancel: Arc<AtomicBool>,
    /// The job under way is a removal, which cannot be canceled.
    mod_removing: bool,
    /// HDR mods found in games' folders that Huedra did not install.
    found: HashMap<GameId, hdr_mods::Found>,
    /// The program found in the folder of a game Huedra has not seen run,
    /// where its mod would go.
    guessed: HashMap<GameId, PathBuf>,
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
    // Mica: Windows tints the window's backdrop (the navigation pane and
    // behind the pages) with the wallpaper, as in its own apps, and the
    // see-through cards and controls (`Theme.card`, `Theme.control-fill`)
    // let it through. Windows 11 22H2 (build 22621) and later; earlier builds
    // would show the transparent backdrop black. Sample builds leave it out
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
    // Looping animations run only while the window has the focus: none in
    // the background or minimized.
    let window_weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, event| {
        if let winit::event::WindowEvent::Focused(focused) = event
            && let Some(ui) = window_weak.upgrade()
        {
            ui.global::<Theme>().set_window_active(*focused);
        }
        EventResult::Propagate
    });
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
    // Screenshots from a sample build name a release, not the workspace's
    // 0.1.0.
    #[cfg(feature = "sample")]
    if let Ok(version) = std::env::var("HUEDRA_SAMPLE_VERSION") {
        ui.set_version(version.into());
    }
    // A sample window can open on the first-run setup.
    #[cfg(feature = "sample")]
    ui.set_show_setup(std::env::var_os("HUEDRA_SAMPLE_SETUP").is_some());
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
    // Someone who used Huedra before this window: once, until closed.
    ui.set_new_look(config.general.disclaimer_accepted && !config.general.new_look_seen);
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
    #[cfg(not(feature = "sample"))]
    let locations_mods = locations.mods.clone();
    let state = State {
        info,
        displays: read_displays().unwrap_or_default(),
        uncalibrated: Vec::new(),
        hdr_peaks: HdrPeaks::parse(
            &huedra_core::files::read_limited(&locations.peaks, 64 << 10)
                .ok()
                .flatten()
                .unwrap_or_default(),
        ),
        profiles: read_profiles(),
        modes: read_modes(),
        games,
        config,
        locations,
        running: Vec::new(),
        icons: HashMap::new(),
        covers: HashMap::new(),
        covers_started: false,
        search: String::new(),
        filter: "all".into(),
        selected: None,
        launch_note: None,
        update_shown: None,
        failure_closed: None,
        failure: None,
        #[cfg(not(feature = "sample"))]
        mods: mod_jobs::load_lists(&locations_mods),
        #[cfg(feature = "sample")]
        mods: sample::mods(),
        #[cfg(not(feature = "sample"))]
        installed: hdr_mods::installed(&locations_mods),
        #[cfg(feature = "sample")]
        installed: sample::installed(),
        // A sample window shows the toggle as on an NVIDIA graphics card.
        rtx_available: cfg!(feature = "sample") || huedra_win::nvapi::available(),
        mod_pending: None,
        mod_plan: None,
        mod_remove: None,
        mod_remove_hand: None,
        mod_busy: None,
        mod_cancel: Arc::new(AtomicBool::new(false)),
        mod_removing: false,
        found: HashMap::new(),
        #[cfg(not(feature = "sample"))]
        guessed: HashMap::new(),
        #[cfg(feature = "sample")]
        guessed: sample::programs(),
    };
    STATE.set(Some(state));
    with_state(note_hdr_peaks);

    with_state(|s| {
        let choices = |names: &[String]| -> ModelRc<SharedString> {
            let list: Vec<SharedString> = std::iter::once("Windows default")
                .chain(names.iter().map(String::as_str))
                .map(SharedString::from)
                .collect();
            ModelRc::new(VecModel::from(list))
        };
        ui.set_sdr_profiles(choices(&s.profiles.sdr));
        let modes: Vec<SharedString> = std::iter::once("As set in Windows".to_owned())
            .chain(s.modes.iter().map(|m| m.label()))
            .map(SharedString::from)
            .collect();
        ui.set_display_modes(ModelRc::new(VecModel::from(modes)));
        ui.set_hdr_profiles(choices(&s.profiles.hdr));
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
    start_mod_lists(&ui);
    start_mod_updates(&ui);
    start_found_scan(&ui);

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
    let _card_art = start_card_art(&ui);
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

/// How bright the HDR layer's waves are: this share of the display's
/// peak, or of `HDR_FALLBACK_NITS` when Windows does not say.
const HDR_PEAK_SHARE: f32 = 0.95;
const HDR_FALLBACK_NITS: f32 = 400.0;
/// The SDR banner's gray waves, readable on each theme.
const WAVES_GRAY_DARK: [u8; 3] = [0xC8, 0xC8, 0xD2];
const WAVES_GRAY_LIGHT: [u8; 3] = [0x5F, 0x5F, 0x6A];

/// Keeps the Status card's banner art in step with the card: the SDR waves
/// (gray, and in color for HDR) are made at the banner's own pixel size and
/// again when it changes (width, scale, theme). On Windows with Mica the HDR
/// layer (`huedra_win::hdr_layer`, off with `HUEDRA_HDR_LAYER=0`) draws the waves over the banner in real HDR while
/// HDR is on and Status is the page, near the peak of the brightest display
/// in HDR. If it cannot be made the SDR waves stay and the log says why.
///
/// The layer follows the card the moment the window reports it moved
/// (`hero-moved`, before the frame is drawn), so it scrolls with the page at
/// any refresh rate; the timer makes the images, which takes longer, and
/// catches HDR turning on or off.
fn start_card_art(ui: &SettingsWindow) -> slint::Timer {
    let art = std::rc::Rc::new(CardArt {
        layer: RefCell::new(None),
        failed: Cell::new(
            !ui.get_mica() || std::env::var_os("HUEDRA_HDR_LAYER").is_some_and(|v| v == "0"),
        ),
        sdr_size: Cell::new((0, 0, false)),
        hdr_size: Cell::new((0, 0, 0)),
        placed: Cell::new(None),
    });
    let weak = ui.as_weak();
    let moved = std::rc::Rc::clone(&art);
    ui.on_hero_moved(move || {
        if let Some(ui) = weak.upgrade() {
            moved.update(&ui, false);
        }
    });
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            if let Some(ui) = weak.upgrade() {
                art.update(&ui, true);
            }
        },
    );
    timer
}

/// The Status card's banner art and the HDR layer over it.
struct CardArt {
    layer: RefCell<Option<HdrLayer>>,
    /// The layer is off, or could not be made.
    failed: Cell<bool>,
    /// The SDR waves' size and theme (dark) as last made.
    sdr_size: Cell<(u32, u32, bool)>,
    /// The HDR waves' size and brightness (nits) as last made.
    hdr_size: Cell<(u32, u32, u32)>,
    /// Where the layer was last put.
    placed: Cell<Option<Option<Placement>>>,
}

impl CardArt {
    /// Brings the art in step with the card; `make` also remakes images
    /// whose size, theme or brightness changed (the timer's turn).
    fn update(&self, ui: &SettingsWindow, make: bool) {
        let scale = ui.window().scale_factor();
        let banner = PixelRect {
            x: (ui.get_hero_x() * scale).round(),
            y: (ui.get_hero_y() * scale).round(),
            width: (ui.get_hero_width() * scale).round(),
            height: (ui.get_hero_banner() * scale).round(),
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "rounded, positive window sizes and nits"
        )]
        let (width, height, nits) = (
            banner.width as u32,
            banner.height as u32,
            (hdr_peak() * HDR_PEAK_SHARE) as u32,
        );
        if width == 0 || height == 0 {
            return;
        }
        let dark = ui.global::<Theme>().get_dark();
        if make && self.sdr_size.get() != (width, height, dark) {
            self.sdr_size.set((width, height, dark));
            let gray = if dark {
                WAVES_GRAY_DARK
            } else {
                WAVES_GRAY_LIGHT
            };
            let theme = ui.global::<Theme>();
            theme.set_waves_gray(rgba_image(
                &icon::sdr_waves(width, height, 0, Some(gray)),
                width,
                height,
            ));
            theme.set_waves_color(rgba_image(
                &icon::sdr_waves(width, height, 0, None),
                width,
                height,
            ));
        }
        if self.failed.get() {
            return;
        }
        let Ok(mut slot) = self.layer.try_borrow_mut() else {
            return;
        };
        if slot.is_none() {
            if !make {
                return;
            }
            let Some(hwnd) = window_handle(ui) else {
                return;
            };
            match HdrLayer::new(hwnd) {
                Ok(made) => *slot = Some(made),
                Err(error) => {
                    self.failed.set(true);
                    log_line(&format!("HDR layer not available: {error}"));
                    return;
                }
            }
        }
        let Some(layer) = slot.as_mut() else {
            return;
        };
        let view = PixelRect {
            x: ui.get_view_x() * scale,
            y: ui.get_view_y() * scale,
            width: ui.get_view_width() * scale,
            height: ui.get_view_height() * scale,
        };
        let wanted = (ui.get_hero_shown() && ui.get_hdr_on()).then_some(Placement {
            rect: banner,
            top_radius: 8.0 * scale,
            bottom_radius: 0.0,
            view,
        });
        if make && wanted.is_some() && self.hdr_size.get() != (width, height, nits) {
            #[expect(clippy::cast_precision_loss, reason = "nits are small")]
            let waves = icon::hdr_waves(width, height, 0, nits as f32);
            match layer.set_image(width, height, &waves) {
                Ok(()) => self.hdr_size.set((width, height, nits)),
                Err(error) => log_line(&format!("HDR layer: {error}")),
            }
        }
        // Not shown before its image is made.
        let ready = self.hdr_size.get().0 > 0;
        let wanted = wanted.filter(|_| ready);
        if self.placed.get() != Some(wanted) {
            match layer.place(wanted) {
                Ok(()) => {
                    self.placed.set(Some(wanted));
                    ui.global::<Theme>().set_hdr_banner(wanted.is_some());
                }
                Err(error) => log_line(&format!("HDR layer: {error}")),
            }
        }
    }
}

/// The peak brightness of the brightest display in HDR, as Windows gives
/// it, or `HDR_FALLBACK_NITS`.
fn hdr_peak() -> f32 {
    with_state(|s| {
        s.displays
            .iter()
            .filter(|d| d.state == HdrState::On)
            .filter_map(|d| d.peak)
            .map(|n| n.get())
            .fold(0.0_f32, f32::max)
    })
    .filter(|&peak| peak > 0.0)
    .unwrap_or(HDR_FALLBACK_NITS)
}

/// The settings window's raw handle, once Windows has made it.
fn window_handle(ui: &SettingsWindow) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    ui.window()
        .with_winit_window(|window| match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        })?
}

/// Adds a line to Huedra's log, with the time.
fn log_line(text: &str) {
    let Some(path) = with_state(|s| s.locations.log.clone()) else {
        return;
    };
    let log = huedra_core::logfile::LogFile::new(path, huedra_core::logfile::MAX_BYTES);
    let _ = log.append(&format!("{} {text}", huedra_win::clock::now()));
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

/// Hands a brightness change to one worker thread, so the steps of a held
/// arrow key apply in order and only the latest per display is sent when
/// several wait; the next display reading shows the result.
fn set_brightness_later(id: DisplayId, brightness: SdrBrightness) {
    type Sender = std::sync::Mutex<std::sync::mpsc::Sender<(DisplayId, SdrBrightness)>>;
    static WORKER: std::sync::OnceLock<Option<Sender>> = std::sync::OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (send, receive) = std::sync::mpsc::channel::<(DisplayId, SdrBrightness)>();
        thread::Builder::new()
            .name("huedra-brightness".into())
            .spawn(move || {
                while let Ok(first) = receive.recv() {
                    let mut latest = vec![first];
                    while let Ok((id, b)) = receive.try_recv() {
                        latest.retain(|(other, _)| *other != id);
                        latest.push((id, b));
                    }
                    for (id, b) in latest {
                        #[cfg(not(feature = "sample"))]
                        let _ = huedra_win::display::set_sdr_brightness(&id, b);
                        #[cfg(feature = "sample")]
                        let _ = (id, b);
                    }
                }
            })
            .ok()
            .map(|_| std::sync::Mutex::new(send))
    });
    if let Some(sender) = worker
        && let Ok(sender) = sender.lock()
    {
        let _ = sender.send((id, brightness));
    }
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
    let weak = ui.as_weak();
    ui.on_close_new_look(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_new_look(false);
            with_state(|s| edit(s, |c| c.general.new_look_seen = true));
        }
    });
    // Setup chooses the mode while the tray may still be starting: the
    // choice is saved as well as sent.
    let weak = ui.as_weak();
    ui.on_setup_mode(move |index| {
        let mode = match index {
            1 => Mode::AlwaysHdr,
            2 => Mode::AlwaysSdr,
            _ => Mode::Automatic,
        };
        if let Some(ui) = weak.upgrade() {
            ui.set_mode(index);
        }
        with_state(|s| edit(s, |c| c.mode = mode));
        send(Request::SetMode { mode });
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

/// The modes the main display offers now.
fn read_modes() -> Vec<DisplayMode> {
    #[cfg(feature = "sample")]
    return [
        "3840x2160@144",
        "3840x2160@120",
        "3840x2160@60",
        "2560x1440@144",
        "1920x1080@60",
    ]
    .iter()
    .filter_map(|m| m.parse().ok())
    .collect();
    #[cfg(not(feature = "sample"))]
    read_displays()
        .unwrap_or_default()
        .iter()
        .find(|d| d.primary)
        .map(|d| {
            huedra_core::mode::for_games(
                huedra_win::mode::offered(&d.id),
                huedra_win::mode::current(&d.id),
            )
        })
        .unwrap_or_default()
}

/// The color profiles installed on this PC.
fn read_profiles() -> Installed {
    #[cfg(feature = "sample")]
    return sample::profiles();
    #[cfg(not(feature = "sample"))]
    huedra_win::color::installed()
}

/// Records the peaks of displays now in HDR in `peaks.toml`, for the game
/// details' peak brightness while they are back in SDR.
fn note_hdr_peaks(state: &mut State) {
    if state.hdr_peaks.note(&state.displays)
        && let Ok(text) = state.hdr_peaks.to_toml()
    {
        let _ = huedra_core::files::write_atomic(&state.locations.peaks, text.as_bytes());
    }
}

/// The HDR displays that have no HDR color profile, as the Windows HDR
/// Calibration app makes. A display whose profile can't be read is left out.
fn read_uncalibrated(displays: &[Display]) -> Vec<DisplayId> {
    #[cfg(feature = "sample")]
    {
        let _ = displays;
        Vec::new()
    }
    #[cfg(not(feature = "sample"))]
    displays
        .iter()
        .filter(|d| d.support.is_capable())
        .filter(|d| {
            matches!(
                huedra_win::color::default_profile(&d.id, huedra_core::color::ColorMode::Hdr),
                Ok(None)
            )
        })
        .map(|d| d.id.clone())
        .collect()
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
            // Color profiles are read every few rounds and when the displays
            // change: the user may have just calibrated one.
            let mut round: u32 = 0;
            let mut last_ids: Vec<DisplayId> = Vec::new();
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
                let ids: Vec<DisplayId> = displays.iter().flatten().map(|d| d.id.clone()).collect();
                let uncalibrated = (round.is_multiple_of(CALIBRATION_ROUNDS) || ids != last_ids)
                    .then(|| read_uncalibrated(displays.as_deref().unwrap_or_default()));
                last_ids = ids;
                round = round.wrapping_add(1);
                tray_seen |= report.is_some();
                let close = tray_seen && tray_gone;
                let delivered = weak.upgrade_in_event_loop(move |ui| {
                    if close {
                        close_with_tray(&ui);
                    }
                    with_state(|s| {
                        let mut changed = false;
                        if let Some(uncalibrated) = uncalibrated
                            && uncalibrated != s.uncalibrated
                        {
                            s.uncalibrated = uncalibrated;
                            changed = true;
                        }
                        if let Some(displays) = displays
                            && displays != s.displays
                        {
                            s.displays = displays;
                            changed = true;
                            note_hdr_peaks(s);
                        }
                        if changed {
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
    ui.set_shortcut_note(
        report
            .map(|r| r.shortcut_note.as_str())
            .unwrap_or_default()
            .into(),
    );
    let Some(report) = report else {
        let on = state.displays.iter().any(|d| d.state == HdrState::On);
        ui.set_hdr_on(on);
        ui.set_headline((if on { "HDR is on" } else { "HDR is off" }).into());
        ui.set_nav_detail("Huedra is not running".into());
        ui.set_detail("Automatic switching is off until Huedra starts.".into());
        ui.set_caption(caption(state, on).into());
        ui.set_history(ModelRc::default());
        ui.set_now(ModelRc::default());
        ui.set_idle("Huedra isn't running.".into());
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
    ui.set_headline(
        (if report.hdr_on {
            "HDR is on"
        } else {
            "HDR is off"
        })
        .into(),
    );
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
        "Added by you"
    } else {
        store.label()
    }
}

/// The line under the state in the navigation pane, for example
/// "Display 1 · Always HDR mode" or "Nothing needs HDR right now".
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
        return "Standard color and brightness".to_owned();
    }
    let peak = state
        .displays
        .iter()
        .filter(|d| d.state == HdrState::On)
        .filter_map(|d| d.peak)
        .map(|n| n.get())
        .fold(0.0_f32, f32::max);
    if peak > 0.0 {
        format!("Wide color · up to {peak:.0} nits")
    } else {
        "Wide color".to_owned()
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
    /// HDR mod: "installed", "available" (on a list) or empty.
    hdr_mod: &'static str,
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
    let hdr = if !state.config.general.game_info || !has_wiki_page(game.store) {
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
            .and_then(|_| wiki_suggestion(state, game))
            .filter(|s| *s != setting),
        hdr_mod: if installed_mod(state, &game.id).is_some() || state.found.contains_key(&game.id) {
            "installed"
        } else if mod_for(state, game).is_some()
            && info.and_then(GameInfo::anticheat_name).is_none()
        {
            // Not offered for games with anti-cheat (see `mod_view`).
            "available"
        } else {
            ""
        },
    }
}

/// PCGamingWiki's suggested setting for a game. None for a game with an
/// HDR mod: the wiki describes the game without it, and the mod adds HDR.
fn wiki_suggestion(state: &State, game: &Game) -> Option<GameHdr> {
    if installed_mod(state, &game.id).is_some() || state.found.contains_key(&game.id) {
        return None;
    }
    state.info.get(&game.id).and_then(|i| i.hdr.suggestion())
}

/// The record of the mod Huedra installed for a game, if any.
fn installed_mod<'a>(state: &'a State, id: &GameId) -> Option<&'a Manifest> {
    state.installed.iter().find(|m| &m.game == id)
}

/// The listed HDR mod for a game, while looking for mods is on.
/// Whether a game of this store is looked up on PCGamingWiki: not one added
/// by hand (its name is a file name) nor an emulator (its games are not the
/// emulator).
fn has_wiki_page(store: Store) -> bool {
    !matches!(store, Store::Manual | Store::Emulator)
}

fn mod_for<'a>(state: &'a State, game: &Game) -> Option<&'a Mod> {
    // An emulator's name never names a game with a mod.
    if !state.config.general.hdr_mods || game.store == Store::Emulator {
        return None;
    }
    hdr_mods::find(&state.mods, &game.name)
}

/// The program of a game a mod goes next to: one Huedra saw run, or else
/// the one found in the game's folder.
fn mod_program(state: &State, game: &Game) -> Option<PathBuf> {
    // Read from the file: the tray process adds a game's program the
    // first time the game runs, which may be while this window is open.
    let saved = Config::load(&state.locations.config).ok();
    let mut programs = saved
        .as_ref()
        .unwrap_or(&state.config)
        .game(&game.id)
        .map(|s| s.programs.clone())
        .unwrap_or_default();
    programs.extend(game.exe.clone());
    mod_jobs::program_of(&programs).or_else(|| state.guessed.get(&game.id).cloned())
}

/// The details pane's HDR mod section: state, title, detail, note, link.
fn mod_view(state: &State, game: &Game) -> (&'static str, String, String, String, String) {
    let link = |m: &Mod| m.page.clone().unwrap_or_default();
    if state.mod_busy.as_ref() == Some(&game.id) {
        let title = mod_for(state, game).map_or_else(
            || "HDR mod".to_owned(),
            |m| format!("{} for {}", m.source.label(), m.name),
        );
        return (
            "working",
            title,
            String::new(),
            String::new(),
            String::new(),
        );
    }
    if let Some(record) = installed_mod(state, &game.id) {
        let mut detail = if record.author.is_empty() {
            "Installed".to_owned()
        } else {
            format!("Installed · by {}", record.author)
        };
        let newer = record.newer && mod_for(state, game).is_some_and(Mod::installable);
        if newer {
            detail.push_str(" · Update available");
        }
        let page = mod_for(state, game).map(link).unwrap_or_default();
        return (
            if newer { "update" } else { "installed" },
            format!("{} for {}", record.source.label(), record.name),
            detail,
            String::new(),
            page,
        );
    }
    if let Some(found) = state.found.get(&game.id) {
        return (
            "found",
            format!("{} is installed", found.source.label()),
            format!(
                "Installed outside Huedra, in {}",
                found.folder.display()
            ),
            "Huedra didn't install it. Remove takes out the mod and its ReShade and keeps a copy of every file.".to_owned(),
            mod_for(state, game).map(link).unwrap_or_default(),
        );
    }
    let Some(entry) = mod_for(state, game) else {
        let note = if !state.config.general.hdr_mods {
            "Looking for HDR mods is off in Settings."
        } else if state.mods.is_empty() {
            "The RenoDX and Luma lists haven't been downloaded yet."
        } else {
            "RenoDX and Luma have no mod for this game."
        };
        return (
            "none",
            String::new(),
            String::new(),
            note.to_owned(),
            String::new(),
        );
    };
    let title = format!("{} for {}", entry.source.label(), entry.name);
    let detail = format!("By {} · {}", entry.author, entry.status.label());
    let anticheat = state.info.get(&game.id).and_then(GameInfo::anticheat_name);
    let (kind, note) = if !entry.installable() {
        ("page", entry.note.clone())
    } else if let Some(name) = anticheat {
        (
            "blocked",
            format!(
                "PCGamingWiki lists {name}. Changed game files can get accounts banned, so Huedra won't install a mod here."
            ),
        )
    } else if mod_program(state, game).is_none() {
        ("program", entry.note.clone())
    } else {
        // Says where it goes, so a wrong guess shows before installing.
        let place = mod_program(state, game)
            .map(|p| format!("Goes next to {}.", p.display()))
            .unwrap_or_default();
        let note = [entry.note.as_str(), place.as_str()]
            .iter()
            .filter(|t| !t.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        ("available", note)
    };
    (kind, title, detail, note, link(entry))
}

/// Whether a game passes the chosen filter: a store, or one of the cues.
fn passes(filter: &str, game: &Game, cues: &Cues) -> bool {
    match filter {
        "all" => true,
        "hdr" => matches!(cues.hdr, "hdr" | "limited"),
        "own" => cues.own,
        "suggest" => cues.suggestion.is_some(),
        "mods" => !cues.hdr_mod.is_empty(),
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
        return "Game info is off".to_owned();
    }
    match state.info.get(id) {
        None => "Not checked yet".to_owned(),
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
                store: match (running, c.hdr_mod) {
                    (true, _) => format!("Running · {store}"),
                    (false, "installed") => format!("{store} · HDR mod installed"),
                    _ => store.to_owned(),
                }
                .into(),
                hdr_mod: c.hdr_mod.into(),
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
                cover: state.covers.get(&g.id).cloned().unwrap_or_default(),
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
    let with_mods = cue_count(&|c| !c.hdr_mod.is_empty());
    if state.config.general.hdr_mods && with_mods > 0 {
        cue_chips.push(StoreChip {
            key: "mods".into(),
            label: "HDR mods".into(),
            count: with_mods,
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
        Store::Emulator,
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
    ui.set_mods_installed(count(state.installed.len()));
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
    let suggestion = wiki_suggestion(state, game);
    let index = |hdr: GameHdr| count(HDR_SETTINGS.iter().position(|h| *h == hdr).unwrap_or(0));
    let delay = settings
        .and_then(|s| s.start_delay)
        .unwrap_or(state.config.general.start_delay);
    let hold = settings
        .and_then(|s| s.hold)
        .unwrap_or(state.config.general.hold);
    let (mod_state, mod_title, mod_detail, mod_note, mod_link) = mod_view(state, game);
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
        sdr_profile: profile_index(
            &state.profiles.sdr,
            settings.and_then(|s| s.sdr_profile.as_deref()),
        ),
        hdr_profile: profile_index(
            &state.profiles.hdr,
            settings.and_then(|s| s.hdr_profile.as_deref()),
        ),
        auto_hdr: state.config.auto_hdr_on(&game.id),
        rtx_hdr: state.config.rtx_hdr_on(&game.id),
        rtx_available: state.rtx_available,
        program_known: settings.is_some_and(|s| !s.programs.is_empty() || s.exe.is_some()),
        mod_state: mod_state.into(),
        mod_title: mod_title.into(),
        mod_detail: mod_detail.into(),
        mod_note: mod_note.into(),
        mod_link: mod_link.into(),
        online: info.is_some_and(|i| i.online),
        // A saved mode the display no longer lists shows as the default
        // until another is chosen; the saved one stays in the file.
        display_mode: settings
            .and_then(|s| s.display_mode)
            .and_then(|m| state.modes.iter().position(|o| *o == m))
            .map_or(0, |i| i32::try_from(i + 1).unwrap_or(0)),
        peak_hint: huedra_core::display::peak_hint(
            &state.displays,
            |d| state.hdr_peaks.game_peak(d),
            |d| state.config.is_managed(d.id.as_str()),
        )
        .unwrap_or_default()
        .into(),
    }
}

/// A saved profile's place in a dropdown list, where 0 is "Windows
/// default" (also shown for a profile that is no longer installed).
fn profile_index(list: &[String], saved: Option<&str>) -> i32 {
    saved
        .and_then(|name| list.iter().position(|p| p.eq_ignore_ascii_case(name)))
        .map_or(0, |i| count(i + 1))
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
        return "Couldn't create the shortcut: Huedra doesn't know which program starts this game."
            .to_owned();
    };
    let Some(desktop) = shell::desktop_folder() else {
        return "Couldn't create the shortcut: the desktop folder wasn't found.".to_owned();
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
        Err(error) => format!("Couldn't create the shortcut: {}.", error.message()),
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
    let weak = ui.as_weak();
    ui.on_games_grid_shown(move || {
        if let Some(ui) = weak.upgrade() {
            start_covers(&ui);
        }
    });
    ui.on_open_wiki(|url| {
        shell::open_web_page(url.as_str());
    });
    let r = refresh.clone();
    let weak = ui.as_weak();
    ui.on_add_game(move || {
        let Some(path) = shell::pick_program("Choose the game's .exe file") else {
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
    ui.on_set_game_display_mode(move |id, index| {
        r(&|s| {
            let mode = usize::try_from(index)
                .ok()
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| s.modes.get(i))
                .copied();
            edit(s, |c| {
                c.game_mut(&GameId::from_raw(id.as_str())).display_mode = mode
            });
        });
    });
    let r = refresh.clone();
    ui.on_set_game_auto_hdr(move |id, on| {
        r(&|s| edit(s, |c| c.set_auto_hdr(&GameId::from_raw(id.as_str()), on)));
    });
    let r = refresh.clone();
    ui.on_set_game_rtx_hdr(move |id, on| {
        r(&|s| edit(s, |c| c.set_rtx_hdr(&GameId::from_raw(id.as_str()), on)));
    });
    wire_mods(ui);
    let r = refresh.clone();
    ui.on_set_game_profile(move |id, hdr, index| {
        r(&|s| {
            let list = if hdr {
                &s.profiles.hdr
            } else {
                &s.profiles.sdr
            };
            // 0 is "Windows default": no profile of the game's own.
            let name = usize::try_from(index)
                .ok()
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| list.get(i))
                .cloned();
            edit(s, |c| {
                let settings = c.game_mut(&GameId::from_raw(id.as_str()));
                if hdr {
                    settings.hdr_profile = name;
                } else {
                    settings.sdr_profile = name;
                }
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

/// Loads the cover grid's art once, the first time the grid is shown. A
/// sample build shows no third-party art.
fn start_covers(ui: &SettingsWindow) {
    if cfg!(feature = "sample") {
        return;
    }
    let Some(games) = with_state(|s| {
        if s.covers_started {
            return None;
        }
        s.covers_started = true;
        Some(s.games.clone())
    })
    .flatten() else {
        return;
    };
    let weak = ui.as_weak();
    crate::covers::spawn(games, move |cover| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                let image = rgba_image(&cover.rgba, cover.width, cover.height);
                s.covers.insert(cover.id, image);
                show_games(&ui, s);
            });
        });
    });
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
            .filter(|g| has_wiki_page(g.store))
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

// ---- HDR mods ---------------------------------------------------------

/// Downloads the RenoDX and Luma lists when they are older than a day.
fn start_mod_lists(ui: &SettingsWindow) {
    if cfg!(feature = "sample") {
        return;
    }
    let Some(folder) =
        with_state(|s| s.config.general.hdr_mods.then(|| s.locations.mods.clone())).flatten()
    else {
        return;
    };
    let weak = ui.as_weak();
    mod_jobs::refresh_lists(folder, move |found| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                s.mods = found;
                show_games(&ui, s);
            });
        });
    });
}

/// Checks the mods Huedra installed for a newer version, at most once a day
/// each, while looking for HDR mods is on.
fn start_mod_updates(ui: &SettingsWindow) {
    if cfg!(feature = "sample") {
        return;
    }
    let Some((records, lists, folder)) = with_state(|s| {
        s.config.general.hdr_mods.then(|| {
            (
                s.installed.clone(),
                s.mods.clone(),
                s.locations.mods.clone(),
            )
        })
    })
    .flatten() else {
        return;
    };
    let weak = ui.as_weak();
    mod_jobs::check_updates(records, lists, folder, move |checked| {
        if checked.is_empty() {
            return;
        }
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                for record in checked {
                    // A record removed or replaced meanwhile stays as it is.
                    if let Some(slot) = s
                        .installed
                        .iter_mut()
                        .find(|m| m.game == record.game && m.installed == record.installed)
                    {
                        *slot = record;
                    }
                }
                show_games(&ui, s);
            });
        });
    });
}

/// Looks for HDR mods put into games' folders by hand, on a background
/// thread, and records them so Auto HDR and RTX HDR stay off for those
/// games too. Games with a mod Huedra installed are left out.
fn start_found_scan(ui: &SettingsWindow) {
    if cfg!(feature = "sample") {
        return;
    }
    let Some(work) = with_state(|s| {
        s.games
            .iter()
            .filter(|g| installed_mod(s, &g.id).is_none())
            .map(|g| {
                let mut folders: Vec<PathBuf> = s
                    .config
                    .game(&g.id)
                    .map(|c| c.programs.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .chain(g.exe.clone())
                    .filter_map(|p| PathBuf::from(p).parent().map(PathBuf::from))
                    .collect();
                // A game with a listed mod whose program Huedra has not seen
                // run gets its program guessed from its folder, so the mod
                // can be installed before the first start.
                let guess =
                    (folders.is_empty() && !g.install_dir.is_empty() && mod_for(s, g).is_some())
                        .then(|| (PathBuf::from(&g.install_dir), g.name.clone()));
                if !g.install_dir.is_empty() {
                    folders.push(PathBuf::from(&g.install_dir));
                }
                (g.id.clone(), folders, guess)
            })
            .collect::<Vec<_>>()
    }) else {
        return;
    };
    let weak = ui.as_weak();
    let _ = thread::Builder::new()
        .name("huedra-found-mods".into())
        .spawn(move || {
            let mut found: HashMap<GameId, hdr_mods::Found> = HashMap::new();
            let mut guessed: HashMap<GameId, PathBuf> = HashMap::new();
            for (id, folders, guess) in work {
                if let Some(f) = hdr_mods::find_in(&folders) {
                    found.insert(id, f);
                } else if let Some((dir, name)) = guess
                    && let Some(program) = huedra_core::icons::find_program(&dir, &[], &name)
                {
                    guessed.insert(id, program);
                }
            }
            let _ = weak.upgrade_in_event_loop(move |ui| {
                with_state(|s| {
                    s.guessed = guessed;
                    let wanted = |id: &GameId| found.get(id).map(|f| f.source.label().to_owned());
                    let ids: Vec<GameId> = s.games.iter().map(|g| g.id.clone()).collect();
                    let changed = ids.iter().any(|id| {
                        s.config.game(id).and_then(|c| c.found_mod.clone()) != wanted(id)
                    });
                    if changed {
                        edit(s, |c| {
                            for id in &ids {
                                let value = wanted(id);
                                if c.game(id).and_then(|g| g.found_mod.clone()) != value {
                                    c.game_mut(id).found_mod = value;
                                }
                            }
                        });
                    }
                    s.found = found;
                    refresh_games(&ui, s);
                });
            });
        });
}

fn lines_model(lines: Vec<String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        lines
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
}

/// The dialog's last step: what happened, and Close.
fn mod_result(ui: &SettingsWindow, title: &str, text: &str, lines: Vec<String>) {
    ui.set_mod_title(title.into());
    ui.set_mod_text(text.into());
    ui.set_mod_changes(lines_model(lines));
    ui.set_mod_dialog(5);
}

/// Shows the Games page again with a fresh details pane, for values that
/// changed outside its controls.
fn refresh_games(ui: &SettingsWindow, state: &State) {
    ui.set_details(ModelRc::default());
    show_games(ui, state);
}

/// Works out the install for a game and asks to confirm it, listing what
/// will change, or says why it cannot be done.
fn offer_install(ui: &SettingsWindow, state: &mut State, id: &GameId, update: bool) {
    let Some(game) = state.games.iter().find(|g| &g.id == id).cloned() else {
        return;
    };
    let Some(entry) = mod_for(state, &game).cloned() else {
        return;
    };
    let Some(program) = mod_program(state, &game) else {
        mod_result(
            ui,
            "Huedra doesn't know the game's program yet",
            "Start the game once so Huedra learns which program to put the mod next to, then try again.",
            Vec::new(),
        );
        return;
    };
    let replaces = if update {
        installed_mod(state, id).cloned()
    } else {
        None
    };
    match mod_jobs::plan(id, &entry, &program) {
        Ok(mut plan) => {
            let verb = if replaces.is_some() {
                "Update"
            } else {
                "Install"
            };
            plan.replaces = replaces;
            let unchecked = if state.config.general.game_info {
                ""
            } else {
                "Game information is off, so Huedra can't check PCGamingWiki for anti-cheat. Make sure this game has none. "
            };
            ui.set_mod_title(format!("{verb} {} for {}?", entry.source.label(), game.name).into());
            ui.set_mod_text(
                format!(
                    "{unchecked}{} by {} ({}). Huedra will:",
                    entry.source.label(),
                    entry.author,
                    entry.status.label().to_lowercase()
                )
                .into(),
            );
            ui.set_mod_changes(lines_model(plan.changes()));
            ui.set_mod_online(state.info.get(id).is_some_and(|i| i.online));
            ui.set_mod_update(plan.replaces.is_some());
            state.mod_plan = Some(plan);
            ui.set_mod_dialog(2);
        }
        Err(why) => mod_result(ui, "This mod can't be installed", &why, Vec::new()),
    }
}

fn wire_mods(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    ui.on_install_mod(move |id| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let id = GameId::from_raw(id.as_str());
        with_state(|s| {
            if s.mod_busy.is_some() {
                return;
            }
            if s.config.general.mods_disclaimer_accepted {
                offer_install(&ui, s, &id, false);
            } else {
                s.mod_pending = Some(id);
                ui.set_mod_dialog(1);
            }
        });
    });
    let weak = ui.as_weak();
    ui.on_mod_accept_notice(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        with_state(|s| {
            edit(s, |c| c.general.mods_disclaimer_accepted = true);
            ui.set_mod_dialog(0);
            if let Some(id) = s.mod_pending.take() {
                offer_install(&ui, s, &id, false);
            }
        });
    });
    let weak = ui.as_weak();
    ui.on_update_mod(move |id| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let id = GameId::from_raw(id.as_str());
        with_state(|s| {
            if s.mod_busy.is_none() {
                offer_install(&ui, s, &id, true);
            }
        });
    });
    let weak = ui.as_weak();
    ui.on_remove_mod(move |id| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let id = GameId::from_raw(id.as_str());
        with_state(|s| {
            if s.mod_busy.is_some() {
                return;
            }
            let Some(record) = installed_mod(s, &id).cloned() else {
                offer_hand_removal(&ui, s, &id);
                return;
            };
            let mut lines: Vec<String> = record
                .files
                .iter()
                .take(8)
                .map(|f| {
                    if f.replaced {
                        format!("Put back the game's own {}", f.path)
                    } else {
                        format!("Delete {}", f.path)
                    }
                })
                .collect();
            if record.files.len() > 8 {
                lines.push(format!("And {} more files", record.files.len() - 8));
            }
            lines.push(
                "Set Windows Auto HDR and NVIDIA RTX HDR back to this game's settings".to_owned(),
            );
            ui.set_mod_title(
                format!("Remove {} from {}?", record.source.label(), record.name).into(),
            );
            ui.set_mod_text(format!("In {}, Huedra will:", record.folder).into());
            ui.set_mod_changes(lines_model(lines));
            ui.set_mod_online(false);
            s.mod_remove = Some(record);
            ui.set_mod_dialog(4);
        });
    });
    let weak = ui.as_weak();
    ui.on_mod_cancel(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        with_state(|s| {
            if ui.get_mod_dialog() == 3 {
                if !s.mod_removing {
                    s.mod_cancel.store(true, Ordering::Relaxed);
                    ui.set_mod_step("Canceling...".into());
                }
                return;
            }
            s.mod_pending = None;
            s.mod_plan = None;
            s.mod_remove = None;
            s.mod_remove_hand = None;
            ui.set_mod_dialog(0);
        });
    });
    let weak = ui.as_weak();
    ui.on_mod_confirm(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        with_state(|s| match ui.get_mod_dialog() {
            2 => start_install(&ui, s),
            4 if s.mod_remove_hand.is_some() => start_hand_removal(&ui, s),
            4 => start_remove(&ui, s),
            _ => {}
        });
    });
}

fn start_install(ui: &SettingsWindow, state: &mut State) {
    let Some(plan) = state.mod_plan.take() else {
        return;
    };
    let game = plan.game.clone();
    let update = plan.replaces.is_some();
    state.mod_busy = Some(game.clone());
    state.mod_removing = false;
    state.mod_cancel = Arc::new(AtomicBool::new(false));
    ui.set_mod_title(
        format!(
            "{} {} for {}",
            if update { "Updating" } else { "Installing" },
            plan.entry.source.label(),
            plan.entry.name
        )
        .into(),
    );
    ui.set_mod_text(SharedString::default());
    ui.set_mod_step("Starting".into());
    ui.set_mod_progress(0.0);
    ui.set_mod_dialog(3);
    refresh_games(ui, state);
    let weak = ui.as_weak();
    let progress = move |step: &str, fraction: f32| {
        let step = SharedString::from(step);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if ui.get_mod_step() != "Canceling..." {
                ui.set_mod_step(step);
            }
            ui.set_mod_progress(fraction);
        });
    };
    let weak = ui.as_weak();
    let done = move |result: Result<Manifest, String>| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                s.mod_busy = None;
                match result {
                    Ok(record) => {
                        let name = format!("{} for {}", record.source.label(), record.name);
                        let source = record.source.label().to_owned();
                        s.installed.retain(|m| m.game != record.game);
                        s.installed.push(record);
                        edit(s, |c| c.game_mut(&game).hdr_mod = Some(source));
                        if update {
                            mod_result(
                                &ui,
                                "HDR mod updated",
                                &format!(
                                    "The newest {name} is in the game's folder. Its settings in the game's ReShade menu (Home) are kept."
                                ),
                                Vec::new(),
                            );
                        } else {
                            mod_result(
                                &ui,
                                "HDR mod installed",
                                &format!(
                                    "{name} is in the game's folder. Start the game and press Home to open its menu. Windows Auto HDR and NVIDIA RTX HDR are off for this game from its next start."
                                ),
                                Vec::new(),
                            );
                        }
                    }
                    Err(why) => {
                        // An update that stopped after taking out the old
                        // version leaves no record of it.
                        let path = hdr_mods::manifest_path(&s.locations.mods, &game);
                        if update && !path.is_file() {
                            s.installed.retain(|m| m.game != game);
                            edit(s, |c| c.game_mut(&game).hdr_mod = None);
                        }
                        let title = if update {
                            "The HDR mod wasn't updated"
                        } else {
                            "The HDR mod wasn't installed"
                        };
                        mod_result(&ui, title, &why, Vec::new());
                    }
                }
                refresh_games(&ui, s);
            });
        });
    };
    mod_jobs::install(
        plan,
        state.locations.mods.clone(),
        state.mod_cancel.clone(),
        progress,
        done,
    );
}

fn start_remove(ui: &SettingsWindow, state: &mut State) {
    let Some(record) = state.mod_remove.take() else {
        return;
    };
    let game = record.game.clone();
    state.mod_busy = Some(game.clone());
    state.mod_removing = true;
    ui.set_mod_title(format!("Removing {} from {}", record.source.label(), record.name).into());
    ui.set_mod_text(SharedString::default());
    ui.set_mod_step("Taking out the mod's files and putting back the game's own".into());
    ui.set_mod_progress(0.5);
    ui.set_mod_dialog(3);
    refresh_games(ui, state);
    let weak = ui.as_weak();
    let done = move |result: Result<Vec<String>, String>| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                s.mod_busy = None;
                s.mod_removing = false;
                match result {
                    Ok(kept) => {
                        s.installed.retain(|m| m.game != game);
                        edit(s, |c| c.game_mut(&game).hdr_mod = None);
                        if kept.is_empty() {
                            mod_result(
                                &ui,
                                "HDR mod removed",
                                "The game's folder is as it was before. Windows Auto HDR and NVIDIA RTX HDR follow this game's settings again.",
                                Vec::new(),
                            );
                        } else {
                            mod_result(
                                &ui,
                                "HDR mod removed, with some files left",
                                "These files changed after the install (a game update or another mod), so Huedra left them. The originals they replaced are kept in Huedra's mods folder.",
                                kept,
                            );
                        }
                    }
                    Err(why) => mod_result(&ui, "The HDR mod wasn't removed", &why, Vec::new()),
                }
                refresh_games(&ui, s);
            });
        });
    };
    mod_jobs::uninstall(record, state.locations.mods.clone(), done);
}

/// Works out what removing a mod installed by hand takes out and asks to
/// confirm it, listing every file and folder.
fn offer_hand_removal(ui: &SettingsWindow, state: &State, id: &GameId) {
    let Some(found) = state.found.get(id).cloned() else {
        return;
    };
    let name = state
        .games
        .iter()
        .find(|g| &g.id == id)
        .map(|g| g.name.clone())
        .unwrap_or_default();
    let id = id.clone();
    let weak = ui.as_weak();
    mod_jobs::plan_by_hand(found.folder.clone(), move |result| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let nothing = with_state(|s| {
                if s.mod_busy.is_some() || ui.get_mod_dialog() != 0 {
                    return false;
                }
                let label = found.source.label();
                match result {
                    Ok(removal) if !removal.items.is_empty() => {
                        let mut lines: Vec<String> = removal
                            .items
                            .iter()
                            .take(8)
                            .map(|item| format!("Take out {item}"))
                            .collect();
                        if removal.items.len() > 8 {
                            lines.push(format!("And {} more", removal.items.len() - 8));
                        }
                        if removal.reshade_kept {
                            lines.push(format!(
                                "Leave ReShade ({}) and its settings: other add-ons in the folder use it",
                                removal.reshade.clone().unwrap_or_default()
                            ));
                        } else if removal.reshade.is_none() {
                            lines.push(
                                "Only the mod's own files go: no ReShade was found next to it"
                                    .to_owned(),
                            );
                        }
                        lines.push(
                            "Keep a copy of everything it takes out in Huedra's folder".to_owned(),
                        );
                        lines.push(
                            "Let Windows Auto HDR and NVIDIA RTX HDR follow this game's settings again"
                                .to_owned(),
                        );
                        ui.set_mod_title(format!("Remove {label} from {name}?").into());
                        ui.set_mod_text(
                            format!(
                                "{label} was installed outside Huedra. In {}, Huedra will:",
                                removal.folder.display()
                            )
                            .into(),
                        );
                        ui.set_mod_changes(lines_model(lines));
                        ui.set_mod_online(false);
                        s.mod_remove_hand = Some((id.clone(), name.clone(), found.source, removal));
                        ui.set_mod_dialog(4);
                        false
                    }
                    Ok(_) => {
                        mod_result(
                            &ui,
                            "Nothing to remove",
                            &format!("The {label} files are no longer in {}.", found.folder.display()),
                            Vec::new(),
                        );
                        true
                    }
                    Err(why) => {
                        mod_result(&ui, "The HDR mod can't be removed", &why, Vec::new());
                        false
                    }
                }
            });
            if nothing == Some(true) {
                start_found_scan(&ui);
            }
        });
    });
}

/// Removes a mod installed by hand once confirmed, keeping a copy of its
/// files, then looks for mods again.
fn start_hand_removal(ui: &SettingsWindow, state: &mut State) {
    let Some((game, name, source, removal)) = state.mod_remove_hand.take() else {
        return;
    };
    state.mod_busy = Some(game.clone());
    state.mod_removing = true;
    ui.set_mod_title(format!("Removing {} from {name}", source.label()).into());
    ui.set_mod_text(SharedString::default());
    ui.set_mod_step("Copying the mod's files to Huedra's folder, then taking them out".into());
    ui.set_mod_progress(0.5);
    ui.set_mod_dialog(3);
    refresh_games(ui, state);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let keep = hdr_mods::removed_folder(&state.locations.mods, &game, now);
    let shown = keep.display().to_string();
    let weak = ui.as_weak();
    let done = move |result: Result<(), String>| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with_state(|s| {
                s.mod_busy = None;
                s.mod_removing = false;
                match result {
                    Ok(()) => {
                        s.found.remove(&game);
                        mod_result(
                            &ui,
                            "HDR mod removed",
                            &format!(
                                "A copy of its files is in {shown}, in case you want them back. Huedra can't tell whether a mod installed by hand replaced any of the game's own files; if the game misbehaves, have Steam or the game's launcher verify its files."
                            ),
                            Vec::new(),
                        );
                    }
                    Err(why) => mod_result(&ui, "The HDR mod wasn't removed", &why, Vec::new()),
                }
                refresh_games(&ui, s);
            });
            start_found_scan(&ui);
        });
    };
    mod_jobs::uninstall_by_hand(removal, keep, done);
}

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
            if let Some(peak) = state.hdr_peaks.game_peak(d).filter(|_| capable) {
                parts.push(format!("Up to {peak}"));
            }
            let mut detail = parts.join(" · ");
            if !detail.is_empty() {
                detail.push_str(". ");
            }
            detail.push_str(if !capable {
                "No HDR support, so Huedra can't change it."
            } else if controlled {
                "Huedra turns HDR on and off here."
            } else {
                "Huedra doesn't change this display. HDR stays as you set it in Windows."
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
                brightness: d
                    .sdr_brightness
                    .map_or(0, |b| i32::try_from(b.nits()).unwrap_or(0)),
                calibrate: capable && state.uncalibrated.contains(&d.id),
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
    // Changed in place when the same displays are listed, so a row being
    // edited (its brightness box) keeps its focus through the readings.
    let current = ui.get_displays();
    if current.row_count() == rows.len() {
        for (i, row) in rows.into_iter().enumerate() {
            if current.row_data(i).as_ref() != Some(&row) {
                current.set_row_data(i, row);
            }
        }
    } else {
        ui.set_displays(ModelRc::new(VecModel::from(rows)));
    }
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
    ui.on_set_brightness(|id, nits| {
        if let Some(brightness) = u32::try_from(nits).ok().and_then(SdrBrightness::from_nits) {
            set_brightness_later(DisplayId::new(id.as_str()), brightness);
        }
    });
    ui.on_open_calibration(|| {
        shell::open_web_page(CALIBRATION_APP);
    });
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
            "RTX Video HDR is on in the NVIDIA app. Any full-screen video with sound turns HDR on, so RTX can convert it."
        }
        Some(false) => "RTX Video HDR is off in the NVIDIA app, so this does nothing for now.",
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
    ui.set_games_auto_hdr(g.auto_hdr);
    ui.set_games_rtx_hdr(g.rtx_hdr);
    ui.set_rtx_available(state.rtx_available);
    ui.set_hdr_mods(g.hdr_mods);
    ui.set_hold(secs(g.hold));
    ui.set_delay(secs(g.start_delay));
    ui.set_hotkeys(g.hotkeys);
    let [hdr, mode, pause] = g
        .shortcuts
        .resolved()
        .map(|k| k.map(|k| k.to_string()).unwrap_or_default());
    ui.set_keys_hdr(hdr.into());
    ui.set_keys_mode(mode.into());
    ui.set_keys_pause(pause.into());
    ui.set_log_titles(g.log_window_titles);
    ui.set_notify_failures(g.notify_failures);
    ui.set_battery_sdr(g.battery_sdr);
}

/// The copy of the settings a restore replaced, beside `config.toml`.
const BEFORE_RESTORE: &str = "config-before-restore.toml";

/// Settings › Advanced › Back up: saves the settings to a file the user
/// chooses and says how it went.
fn back_up_settings(ui: &SettingsWindow) {
    let Some(file) =
        shell::pick_settings_file("Back up Huedra's settings", "Huedra settings.toml", true)
    else {
        return;
    };
    let note = with_state(|s| {
        if let Ok(config) = Config::load(&s.locations.config) {
            s.config = config;
        }
        match s.config.to_toml().map(|text| {
            huedra_core::files::write_atomic(std::path::Path::new(&file), text.as_bytes())
        }) {
            Ok(Ok(())) => format!("Settings saved to {file}."),
            Ok(Err(error)) => format!("Couldn't save the settings: {error}."),
            Err(error) => format!("Couldn't save the settings: {error}."),
        }
    })
    .unwrap_or_default();
    ui.set_backup_note(note.into());
}

/// Settings › Advanced › Restore: replaces the settings with a saved file's,
/// keeping the current ones in [`BEFORE_RESTORE`], and shows them.
fn restore_settings(ui: &SettingsWindow) {
    let Some(file) = shell::pick_settings_file("Restore Huedra's settings", "", false) else {
        return;
    };
    let read = huedra_core::files::read_limited(std::path::Path::new(&file), 1 << 20)
        .map_err(|e| e.to_string())
        .and_then(|text| text.ok_or_else(|| "the file is empty or missing".to_owned()))
        .and_then(|text| Config::parse(&text).map_err(|e| e.to_string()));
    let config = match read {
        Ok(config) => config,
        Err(why) => {
            ui.set_backup_note(format!("Nothing was changed: {file} isn't usable ({why}).").into());
            return;
        }
    };
    let note = with_state(|s| {
        let before = s
            .locations
            .config
            .parent()
            .map(|dir| dir.join(BEFORE_RESTORE));
        if let Some(before) = &before
            && s.locations.config.is_file()
            && let Err(error) = std::fs::copy(&s.locations.config, before)
        {
            return format!("Nothing was changed: the current settings couldn't be kept ({error}).");
        }
        if let Err(error) = config.save(&s.locations.config) {
            return format!("Nothing was changed: {error}.");
        }
        s.config = config;
        thread::spawn(|| {
            let _ = tray(&Request::Reload);
        });
        refresh_games(ui, s);
        show_video(ui, s);
        show_displays(ui, s);
        show_settings(ui, s);
        format!("Settings restored from {file}. The ones they replaced are in {BEFORE_RESTORE} in Huedra's folder.")
    })
    .unwrap_or_default();
    ui.set_backup_note(note.into());
}

fn wire_settings(ui: &SettingsWindow) {
    let weak = ui.as_weak();
    ui.on_back_up_settings(move || {
        if let Some(ui) = weak.upgrade() {
            back_up_settings(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_restore_settings(move || {
        if let Some(ui) = weak.upgrade() {
            restore_settings(&ui);
        }
    });
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
    // Defaults for games; games that follow them change too, so the
    // details pane is drawn again.
    for (setter, which) in [(0, "auto"), (1, "rtx"), (2, "mods")] {
        let c = change.clone();
        let weak = ui.as_weak();
        let apply = move |on: bool| {
            c(&|c| match which {
                "auto" => c.general.auto_hdr = on,
                "rtx" => c.general.rtx_hdr = on,
                _ => c.general.hdr_mods = on,
            });
            if let Some(ui) = weak.upgrade() {
                with_state(|s| {
                    show_settings(&ui, s);
                    refresh_games(&ui, s);
                });
                if which == "mods" && on {
                    start_mod_lists(&ui);
                }
            }
        };
        match setter {
            0 => ui.on_set_games_auto_hdr(apply),
            1 => ui.on_set_games_rtx_hdr(apply),
            _ => ui.on_set_hdr_mods(apply),
        }
    }
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
    let c = change.clone();
    let weak = ui.as_weak();
    ui.on_set_shortcut(move |index, text| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        match Binding::parse(&text) {
            Ok(binding) => {
                ui.set_shortcut_error("".into());
                c(&|c| {
                    let binding = binding.clone();
                    let s = &mut c.general.shortcuts;
                    match index {
                        0 => s.toggle_hdr = binding,
                        1 => s.cycle_mode = binding,
                        _ => s.pause = binding,
                    }
                });
            }
            Err(error) => {
                ui.set_shortcut_error(format!("\"{}\" wasn't saved: {error}", text.trim()).into())
            }
        }
    });
    let c = change.clone();
    ui.on_set_log_titles(move |on| {
        c(&|c| c.general.log_window_titles = on);
    });
    let c = change.clone();
    ui.on_set_battery_sdr(move |on| {
        c(&|c| c.general.battery_sdr = on);
    });
    ui.on_set_notify_failures(move |on| {
        change(&|c| c.general.notify_failures = on);
    });
}

// ---- Window geometry --------------------------------------------------

fn restore_geometry(ui: &SettingsWindow) {
    let Some(geometry) = with_state(|s| WindowGeometry::load(&s.locations.window)).flatten() else {
        fit_first_size(ui);
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

/// The first window keeps its default size (wide enough for Status's two
/// cards side by side) unless that leaves no room on a small screen: then
/// it takes nine tenths of the main display's work area.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "window sizes are small and positive"
)]
fn fit_first_size(ui: &SettingsWindow) {
    let Some(screen) = huedra_win::monitor::at(0, 0) else {
        return;
    };
    // `preferred-width` and `preferred-height` in app.slint.
    let (width, height) = (1100.0_f32, 780.0_f32);
    let wanted = |side: f32| (side * screen.scale).round() as u32;
    let room = |side: i32| (f64::from(side) * 0.9) as u32;
    let (wanted_width, wanted_height) = (wanted(width), wanted(height));
    let (room_width, room_height) = (room(screen.work.width()), room(screen.work.height()));
    if wanted_width > room_width || wanted_height > room_height {
        ui.window().set_size(PhysicalSize::new(
            wanted_width.min(room_width),
            wanted_height.min(room_height),
        ));
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
                    "Couldn't check for updates. The log has details.".to_owned()
                } else if waiting.is_empty() {
                    format!("Huedra {} is up to date.", huedra_core::VERSION)
                } else {
                    format!("Huedra {waiting} is available.")
                }
                .into(),
            );
            let mut note = update_note(&path);
            if !ok {
                note = format!("Couldn't check for updates. The log has details. {note}");
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
            "Installing… Huedra restarts in the tray when done."
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
                ui.set_update_status("Installed. The window reopens with the new version.".into());
                REOPEN.set(true);
                let _ = slint::quit_event_loop();
            } else if ok {
                ui.set_update_status("Already up to date.".into());
            } else {
                ui.set_update_status(
                    "Couldn't install the update. If a game or video is using HDR, try again when it ends. Otherwise, the log has details."
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
        // The first acceptance: three short screens of setup follow.
        ui.set_show_setup(true);
        let setup = with_state(|s| {
            edit(s, |c| {
                c.general.disclaimer_accepted = true;
                // A new user gets the setup, not the notice about the new
                // window.
                c.general.new_look_seen = true;
            });
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

    if cfg!(feature = "sample") {
        return "Last checked an hour ago.".to_owned();
    }
    if !update::is_available() {
        return "This build can't check for updates.".to_owned();
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
