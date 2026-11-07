//! The quick panel: a small window without a frame that opens beside the
//! tray icon (`huedra flyout`, started by a click on the icon). It shows the
//! state, the mode, each display with its HDR switch, Pause and "Never use HDR for"
//! the game running, and closes when it loses the focus or on Escape.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use huedra_core::config::Config;
use huedra_core::display::{Display, HdrState};
use huedra_core::engine::Mode;
use huedra_core::flyout::{self, Rect};
use huedra_core::ipc::{HistoryKind, Request, Response, StatusReport};
use huedra_core::locations::Locations;
use huedra_win::{instance, monitor, registry, shell};
use slint::winit_030::{EventResult, WinitWindowAccessor as _, winit};
use slint::{ComponentHandle, Model, ModelRc, PhysicalPosition, VecModel};

use crate::app::{read_displays, tray};
#[cfg(feature = "sample")]
use crate::sample;
use crate::ui::{FlyDisplay, FlyoutWindow, Theme};

/// The panel's window title: not shown, but read by screen readers, and how
/// the tray finds the panel to close it.
pub const TITLE: &str = "Huedra quick panel";
/// Held by the process showing the panel, so only one opens.
const CLAIM: &str = "Huedra-flyout";
/// The panel's width and its distance from the screen edges, in effective
/// pixels (as at 100 % scale).
const WIDTH: f32 = 360.0;
const MARGIN: f32 = 12.0;
/// How often the panel asks the tray for its state while it is open.
const POLL: Duration = Duration::from_secs(1);
/// After losing the focus the panel is hidden at once but its process
/// stays this long: a click on the tray icon that took the focus then finds
/// it still open and closes it instead of opening it again.
const LINGER: Duration = Duration::from_millis(400);

/// Whether a quick panel is open (or closing).
pub fn is_open() -> bool {
    instance::claim_window(CLAIM).is_none()
}

/// Where the panel belongs: the tray icon and the monitor it is on.
#[derive(Debug, Clone, Copy)]
struct Place {
    icon: Rect,
    monitor: monitor::Monitor,
}

/// A sample build can keep the panel open when it loses the focus
/// (`HUEDRA_SAMPLE_PINNED`), for driving it in tests.
fn pinned() -> bool {
    cfg!(feature = "sample") && std::env::var_os("HUEDRA_SAMPLE_PINNED").is_some()
}

/// Effective pixels to physical pixels.
#[expect(
    clippy::cast_possible_truncation,
    reason = "screen sizes are far inside i32"
)]
fn physical(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
}

/// The panel's top-left corner for its height now.
fn corner(ui: &FlyoutWindow, place: &Place) -> (i32, i32) {
    let scale = place.monitor.scale;
    let height = ui.get_wanted_height().min(ui.get_room_height());
    flyout::place(
        place.icon,
        place.monitor.work,
        physical(WIDTH, scale),
        physical(height, scale),
        physical(MARGIN, scale),
    )
}

/// Opens the panel beside the tray icon at `icon` (or the pointer) and runs
/// it until it closes. Does nothing when one is open already.
///
/// # Errors
/// Slint platform errors (no graphics or window system).
pub fn run(locations: Locations, icon: Option<Rect>) -> Result<(), slint::PlatformError> {
    let Some(_claim) = instance::claim_window(CLAIM) else {
        return Ok(());
    };
    let icon = icon.or_else(|| {
        monitor::pointer().map(|(x, y)| Rect {
            left: x,
            top: y,
            right: x + 1,
            bottom: y + 1,
        })
    });
    let place = icon.and_then(|icon| {
        monitor::at(icon.left + icon.width() / 2, icon.top + icon.height() / 2)
            .map(|monitor| Place { icon, monitor })
    });

    // The window is made when it is shown; its place is known by then.
    let position: Arc<Mutex<Option<(i32, i32)>>> = Arc::default();
    let at = position.clone();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(move |attributes| {
            use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows as _};
            let attributes = attributes
                .with_skip_taskbar(true)
                .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
                .with_corner_preference(CornerPreference::Round)
                .with_undecorated_shadow(true)
                .with_resizable(false);
            match *at.lock().unwrap_or_else(PoisonError::into_inner) {
                Some((x, y)) => attributes.with_position(winit::dpi::PhysicalPosition::new(x, y)),
                None => attributes,
            }
        })
        .select()?;

    let ui = FlyoutWindow::new()?;
    ui.global::<Theme>().set_motion(shell::animations_enabled());
    ui.set_light_theme(registry::apps_use_light_theme());
    #[cfg(feature = "sample")]
    {
        sample::start(locations.config.clone(), huedra_win::clock::now());
        if let Ok(theme) = std::env::var("HUEDRA_SAMPLE_THEME") {
            ui.set_light_theme(theme == "light");
        }
    }
    show(&ui, &snapshot(&locations.config));
    if let Some(place) = &place {
        let room = f32::from(u16::try_from(place.monitor.work.height()).unwrap_or(u16::MAX))
            / place.monitor.scale
            - 2.0 * MARGIN;
        ui.set_room_height(room.max(200.0));
        *position.lock().unwrap_or_else(PoisonError::into_inner) = Some(corner(&ui, place));
    }
    wire(&ui, &locations, place);

    // Light dismiss, as Quick Settings: losing the focus closes the panel,
    // once it has had it.
    let focused = std::cell::Cell::new(false);
    ui.window().on_winit_window_event(move |window, event| {
        match event {
            winit::event::WindowEvent::Focused(true) => focused.set(true),
            winit::event::WindowEvent::Focused(false) if focused.get() && !pinned() => {
                let _ = window.hide();
                slint::Timer::single_shot(LINGER, || {
                    let _ = slint::quit_event_loop();
                });
            }
            _ => {}
        }
        EventResult::Propagate
    });
    // Alt+F4 or a close message: the process ends with the panel.
    ui.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    ui.show()?;
    // Opened by a click on the tray icon, the panel may take the focus.
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::ZERO, move || {
        if let Some(ui) = weak.upgrade() {
            ui.window()
                .with_winit_window(winit::window::Window::focus_window);
        }
    });
    start_polling(&ui, locations.config.clone());
    slint::run_event_loop_until_quit()
}

/// What the panel shows, read off the window's thread.
struct Snapshot {
    report: Option<StatusReport>,
    displays: Vec<Display>,
    config: Config,
}

fn snapshot(config: &Path) -> Snapshot {
    Snapshot {
        report: match tray(&Request::Status) {
            Ok(Response::Status(report)) => Some(report),
            _ => None,
        },
        displays: read_displays().unwrap_or_default(),
        config: Config::load(config).unwrap_or_default(),
    }
}

/// "Cyberpunk 2077 · on since 21:14", "Nothing needs HDR right now".
fn detail(report: &StatusReport) -> String {
    let since = report
        .history
        .iter()
        .find(|h| h.kind == HistoryKind::Hdr)
        .map(|h| h.time.as_str())
        .filter(|_| report.hdr_on && report.paused_minutes.is_none());
    match since {
        Some(time) => format!(
            "{} · on since {time}",
            report.reason.trim_end_matches(" is running")
        ),
        None => report.reason.clone(),
    }
}

fn show(ui: &FlyoutWindow, snapshot: &Snapshot) {
    let report = snapshot.report.as_ref();
    ui.set_tray_running(report.is_some());
    ui.set_hdr_on(report.map_or_else(
        || snapshot.displays.iter().any(|d| d.state == HdrState::On),
        |r| r.hdr_on,
    ));
    ui.set_detail(
        report
            .map_or_else(|| "Huedra isn't running.".to_owned(), detail)
            .into(),
    );
    if let Some(report) = report {
        ui.set_mode(match report.mode {
            Mode::Automatic => 0,
            Mode::AlwaysHdr => 1,
            Mode::AlwaysSdr => 2,
        });
        ui.set_paused(report.paused_minutes.is_some());
        ui.set_never(report.never.as_str().into());
    } else {
        ui.set_never("".into());
    }
    // Displays without HDR are left out; those Huedra does not control are
    // listed without a switch.
    let rows: Vec<FlyDisplay> = snapshot
        .displays
        .iter()
        .filter(|d| d.support.is_capable())
        .map(|d| {
            let on = d.state == HdrState::On;
            let controlled = snapshot.config.is_managed(d.id.as_str());
            FlyDisplay {
                id: d.id.as_str().into(),
                name: if d.name.trim().is_empty() {
                    format!("Display {}", d.number)
                } else {
                    format!("Display {} · {}", d.number, d.name)
                }
                .into(),
                line: format!(
                    "{} · {}",
                    if on { "HDR on" } else { "HDR off" },
                    if controlled {
                        "controlled by Huedra"
                    } else {
                        "not controlled by Huedra"
                    }
                )
                .into(),
                controlled,
                hdr_on: on,
            }
        })
        .collect();
    // Replaced only when something changed, so the switch being used keeps
    // the keyboard and the pointer.
    if !ui.get_displays().iter().eq(rows.iter().cloned()) {
        ui.set_displays(ModelRc::new(VecModel::from(rows)));
    }
}

/// Sends a request off the window's thread, then shows the new state.
fn send(ui: &FlyoutWindow, config: PathBuf, request: Request) {
    let weak = ui.as_weak();
    thread::spawn(move || {
        let _ = tray(&request);
        let snapshot = snapshot(&config);
        let _ = weak.upgrade_in_event_loop(move |ui| show(&ui, &snapshot));
    });
}

fn wire(ui: &FlyoutWindow, locations: &Locations, place: Option<Place>) {
    let config = locations.config.clone();
    let weak = ui.as_weak();
    let c = config.clone();
    ui.on_set_mode(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mode = match index {
            1 => Mode::AlwaysHdr,
            2 => Mode::AlwaysSdr,
            _ => Mode::Automatic,
        };
        // Shown at once, so arrow keys step on from the new mode.
        ui.set_mode(index);
        send(&ui, c.clone(), Request::SetMode { mode });
    });
    let weak = ui.as_weak();
    let c = config.clone();
    ui.on_toggle_pause(move || {
        if let Some(ui) = weak.upgrade() {
            let request = if ui.get_paused() {
                Request::Resume
            } else {
                Request::Pause { minutes: 60 }
            };
            send(&ui, c.clone(), request);
        }
    });
    let weak = ui.as_weak();
    let c = config.clone();
    ui.on_set_display(move |id, on| {
        if let Some(ui) = weak.upgrade() {
            let request = Request::SetDisplayHdr {
                display: id.to_string(),
                on,
            };
            send(&ui, c.clone(), request);
        }
    });
    let weak = ui.as_weak();
    ui.on_never_for(move || {
        if let Some(ui) = weak.upgrade() {
            send(&ui, config.clone(), Request::NeverForActive);
        }
    });
    ui.on_open_huedra(|| {
        instance::pass_on_foreground();
        if let Ok(exe) = std::env::current_exe() {
            huedra_win::process::spawn_detached(&exe, &["settings"]);
        }
        let _ = slint::quit_event_loop();
    });
    let weak = ui.as_weak();
    let paths = locations.clone();
    ui.on_report(move || {
        let copied = !cfg!(feature = "sample") && huedra_win::report::copy_details(&paths);
        if let Some(ui) = weak.upgrade() {
            ui.set_report_note(
                (if copied {
                    "Copied to the clipboard"
                } else {
                    "Could not copy the details"
                })
                .into(),
            );
        }
    });
    ui.on_dismiss(|| {
        let _ = slint::quit_event_loop();
    });
    // The panel grows or shrinks (a "Never use HDR for" button, a display more):
    // its edge stays by the taskbar.
    let weak = ui.as_weak();
    ui.on_resized(move || {
        if let (Some(ui), Some(place)) = (weak.upgrade(), &place) {
            let (x, y) = corner(&ui, place);
            ui.window().set_position(PhysicalPosition::new(x, y));
        }
    });
}

/// Follows the tray while the panel is open; closes the panel when the
/// tray ends.
fn start_polling(ui: &FlyoutWindow, config: PathBuf) {
    let weak = ui.as_weak();
    let _ = thread::Builder::new()
        .name("huedra-flyout".into())
        .spawn(move || {
            let mut tray_seen = false;
            loop {
                thread::sleep(POLL);
                let snapshot = snapshot(&config);
                let gone = tray_seen && snapshot.report.is_none();
                tray_seen |= snapshot.report.is_some();
                let delivered = weak.upgrade_in_event_loop(move |ui| {
                    if gone {
                        let _ = slint::quit_event_loop();
                    }
                    show(&ui, &snapshot);
                });
                if delivered.is_err() {
                    return;
                }
            }
        });
}
