//! HDR from Steam's Play button: Steam writes the id of the game it starts
//! to `HKCU\Software\Valve\Steam\RunningAppID` within about a second of
//! starting the game's first program (0.7 s after it, measured), long
//! before the game's window shows. A Steam game with "Turn HDR on when
//! Steam starts it" gets HDR on at that moment, for games that check HDR
//! before their window shows (Battlefield 6) and that anti-cheat stops when
//! they are started through `huedra launch`. Nothing touches the game
//! itself.
//!
//! The early start lasts until the game ends. It follows the game's display
//! once the game's own window is found. When Steam stops reporting the game
//! before it was ever seen, it ends after [`GRACE`].
//! Feature doc: `docs/features/06-launch-with-hdr.md`.

use std::time::{Duration, Instant};

use crate::engine::TriggerId;
use crate::library::{GameId, Store};

const TRIGGER_PREFIX: &str = "steam-start:";

/// How long an early start outlives Steam reporting the game when the game
/// never appeared: Steam stops reporting a game whose first program hands
/// over to another launcher (the EA app) that starts the real game later.
pub const GRACE: Duration = Duration::from_secs(60);

/// The trigger of an early start for a Steam app id.
pub fn trigger_id(app_id: u32) -> TriggerId {
    TriggerId::new(format!("{TRIGGER_PREFIX}{app_id}"))
}

/// Whether a trigger is an early start rather than a game, a video or a
/// launch.
pub fn is_steam_start(id: &TriggerId) -> bool {
    id.as_str().starts_with(TRIGGER_PREFIX)
}

/// The library id of a Steam app.
pub fn game_id(app_id: u32) -> GameId {
    GameId::new(Store::Steam, &app_id.to_string())
}

/// One early start in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EarlyStart {
    app_id: u32,
    /// When Steam stopped reporting the game, while it still does not.
    stopped: Option<Instant>,
}

/// The early starts in progress and the app Steam reports.
#[derive(Debug, Default)]
pub struct SteamStarts {
    starts: Vec<EarlyStart>,
    running: u32,
}

impl SteamStarts {
    /// Begins with the app Steam reports when Huedra starts; a game
    /// already running then gets no early start.
    pub fn new(running: u32) -> Self {
        Self {
            starts: Vec::new(),
            running,
        }
    }

    /// Steam now reports `app_id` (0 when no game). Returns the app id that
    /// Steam has just started and that has no early start yet; the caller
    /// checks the game's setting and calls [`Self::started`].
    pub fn steam_reports(&mut self, app_id: u32, now: Instant) -> Option<u32> {
        if app_id == self.running {
            return None;
        }
        self.running = app_id;
        for start in &mut self.starts {
            if start.app_id == app_id {
                start.stopped = None;
            } else {
                start.stopped.get_or_insert(now);
            }
        }
        (app_id != 0 && !self.is_starting(app_id)).then_some(app_id)
    }

    /// Records an early start the caller turned on.
    pub fn started(&mut self, app_id: u32) {
        if !self.is_starting(app_id) {
            self.starts.push(EarlyStart {
                app_id,
                stopped: None,
            });
        }
    }

    /// Whether `app_id` has an early start in progress.
    pub fn is_starting(&self, app_id: u32) -> bool {
        self.starts.iter().any(|s| s.app_id == app_id)
    }

    /// The game ended: its early start ends with it. Returns whether it had
    /// one.
    pub fn game_ended(&mut self, app_id: u32) -> bool {
        let before = self.starts.len();
        self.starts.retain(|s| s.app_id != app_id);
        self.starts.len() != before
    }

    /// Ends the early starts whose game Steam stopped reporting at least
    /// [`GRACE`] ago and that `game_runs` says never appeared. A game that
    /// runs keeps its early start until [`Self::game_ended`].
    pub fn expire(&mut self, now: Instant, game_runs: impl Fn(u32) -> bool) -> Vec<u32> {
        let mut ended = Vec::new();
        self.starts.retain(|s| {
            let due = s
                .stopped
                .is_some_and(|at| now.saturating_duration_since(at) >= GRACE);
            let end = due && !game_runs(s.app_id);
            if end {
                ended.push(s.app_id);
            }
            !end
        });
        ended
    }

    /// When [`Self::expire`] should run next: the earliest grace end among
    /// starts that Steam stopped reporting.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.starts
            .iter()
            .filter_map(|s| s.stopped.map(|at| at + GRACE))
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BF6: u32 = 2_807_960;

    #[test]
    fn ids() {
        let id = trigger_id(BF6);
        assert_eq!(id.as_str(), "steam-start:2807960", "key");
        assert!(is_steam_start(&id), "recognized");
        assert!(!is_steam_start(&TriggerId::new("steam:2807960")), "a game");
        assert!(!is_steam_start(&TriggerId::new("launch:42")), "a launch");
        assert_eq!(game_id(BF6).as_str(), "steam:2807960", "library id");
    }

    #[test]
    fn play_starts_once() {
        let now = Instant::now();
        let mut starts = SteamStarts::new(0);
        assert_eq!(starts.steam_reports(BF6, now), Some(BF6), "play");
        starts.started(BF6);
        assert!(starts.is_starting(BF6), "in progress");
        assert_eq!(starts.steam_reports(BF6, now), None, "same value again");
        assert_eq!(starts.steam_reports(0, now), None, "stopped");
        assert_eq!(
            starts.steam_reports(BF6, now),
            None,
            "reported again while in progress"
        );
    }

    #[test]
    fn running_at_start_is_left_alone() {
        let mut starts = SteamStarts::new(BF6);
        assert_eq!(starts.steam_reports(BF6, Instant::now()), None);
    }

    #[test]
    fn a_game_that_never_appears_ends_after_the_grace() {
        let t0 = Instant::now();
        let mut starts = SteamStarts::new(0);
        starts.steam_reports(BF6, t0);
        starts.started(BF6);
        assert_eq!(
            starts.next_deadline(),
            None,
            "no deadline while Steam reports it"
        );
        let t1 = t0 + Duration::from_secs(2);
        starts.steam_reports(0, t1);
        assert_eq!(starts.next_deadline(), Some(t1 + GRACE), "deadline");
        assert!(
            starts
                .expire(t1 + GRACE - Duration::from_secs(1), |_| false)
                .is_empty(),
            "too early"
        );
        assert_eq!(starts.expire(t1 + GRACE, |_| false), [BF6], "ended");
        assert!(!starts.is_starting(BF6), "gone");
    }

    #[test]
    fn a_running_game_keeps_its_start_until_it_ends() {
        let t0 = Instant::now();
        let mut starts = SteamStarts::new(0);
        starts.steam_reports(BF6, t0);
        starts.started(BF6);
        starts.steam_reports(0, t0);
        assert!(starts.expire(t0 + GRACE, |_| true).is_empty(), "game runs");
        assert!(starts.is_starting(BF6), "kept");
        assert!(starts.game_ended(BF6), "ends with the game");
        assert!(!starts.game_ended(BF6), "only once");
    }

    #[test]
    fn steam_reporting_again_cancels_the_grace() {
        let t0 = Instant::now();
        let mut starts = SteamStarts::new(0);
        starts.steam_reports(BF6, t0);
        starts.started(BF6);
        starts.steam_reports(0, t0);
        starts.steam_reports(BF6, t0);
        assert_eq!(starts.next_deadline(), None, "grace canceled");
        assert!(starts.expire(t0 + GRACE, |_| false).is_empty(), "kept");
    }

    #[test]
    fn another_game_starts_the_grace_of_the_first() {
        let t0 = Instant::now();
        let mut starts = SteamStarts::new(0);
        starts.steam_reports(BF6, t0);
        starts.started(BF6);
        assert_eq!(
            starts.steam_reports(1_091_500, t0),
            Some(1_091_500),
            "second"
        );
        starts.started(1_091_500);
        assert_eq!(starts.next_deadline(), Some(t0 + GRACE), "first stopped");
        assert!(starts.is_starting(BF6) && starts.is_starting(1_091_500));
    }
}
