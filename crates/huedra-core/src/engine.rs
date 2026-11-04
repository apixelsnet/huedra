//! Decides, per managed display, when HDR should be on.
//!
//! No Windows code: the caller passes every event with the current time,
//! carries out the returned [`Action`]s and sets a timer for
//! [`Engine::next_deadline`]. Principal rule: Huedra turns off only the HDR
//! that it turned on. Feature doc: `docs/features/02-engine.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::display::{DisplayId, HdrState};

/// How long to wait before retrying a display that refused to return to SDR.
const OFF_RETRY: Duration = Duration::from_secs(30);

/// The user's choice of how HDR is controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Follow the rules for each game and player.
    #[default]
    Automatic,
    /// Keep every managed display in HDR.
    AlwaysHdr,
    /// Never turn HDR on.
    AlwaysSdr,
}

impl Mode {
    /// The next mode in the order used by the mode shortcut.
    pub fn next(self) -> Self {
        match self {
            Self::Automatic => Self::AlwaysHdr,
            Self::AlwaysHdr => Self::AlwaysSdr,
            Self::AlwaysSdr => Self::Automatic,
        }
    }

    /// Name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::AlwaysHdr => "Always HDR",
            Self::AlwaysSdr => "Always SDR",
        }
    }
}

/// Identifies one cause of HDR, such as a game, across its processes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TriggerId(String);

impl TriggerId {
    /// Wraps a key such as `steam:1091500`.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Something that wants HDR while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Name used in reasons, for example "Cyberpunk 2077".
    pub label: String,
    /// How long it must run before HDR is turned on.
    pub delay: Duration,
    /// How long HDR stays on after it ends.
    pub hold: Duration,
    /// The displays it wants in HDR.
    pub target: Target,
}

/// Which managed displays a trigger wants in HDR.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Target {
    /// Every managed display.
    #[default]
    All,
    /// Only these displays, such as the one showing the game or video.
    /// Empty while its window is not known yet: nothing switches.
    Only(BTreeSet<DisplayId>),
}

impl Target {
    /// Whether the trigger wants this display in HDR.
    pub fn covers(&self, display: &DisplayId) -> bool {
        match self {
            Self::All => true,
            Self::Only(displays) => displays.contains(display),
        }
    }
}

/// Where a managed display stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// SDR, and Huedra has no reason to change it.
    Sdr,
    /// A trigger is running; HDR turns on at `until` unless it ends first.
    Waiting {
        /// When the start delay elapses.
        until: Instant,
    },
    /// HDR is on because Huedra turned it on.
    HdrByHuedra,
    /// The last trigger ended; HDR turns off at `until` unless a new one starts.
    Holding {
        /// When the hold time elapses.
        until: Instant,
    },
    /// HDR was turned on outside Huedra (or by the user's shortcut) and is
    /// never turned off automatically.
    HdrExternal,
}

impl Phase {
    /// Whether HDR in this phase belongs to Huedra.
    pub fn is_owned(self) -> bool {
        matches!(self, Self::HdrByHuedra | Self::Holding { .. })
    }
}

/// A change for the caller to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// The display to change.
    pub display: DisplayId,
    /// The state to set.
    pub state: HdrState,
}

/// What happened, for the history list and the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// HDR was turned on.
    Enabled,
    /// HDR was turned off.
    Disabled,
    /// A change outside Huedra was noticed.
    External,
    /// Windows did not apply a change.
    Failed,
}

/// One entry for the history list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The display concerned.
    pub display: DisplayId,
    /// What happened.
    pub kind: NoteKind,
    /// Why, in a full sentence fragment such as "Cyberpunk 2077 is running".
    pub reason: String,
}

/// The result of one engine call.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Changes to apply, in order.
    pub actions: Vec<Action>,
    /// Entries for the history list.
    pub notes: Vec<Note>,
}

/// A summary for the tray icon and the settings window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Current mode.
    pub mode: Mode,
    /// End of the pause, if paused.
    pub paused_until: Option<Instant>,
    /// Whether any managed display is in HDR.
    pub hdr_on: bool,
    /// The trigger keeping HDR on or about to turn it on.
    pub reason: Option<String>,
    /// Each managed display and its phase.
    pub displays: Vec<(DisplayId, Phase)>,
}

#[derive(Debug)]
struct Active {
    trigger: Trigger,
    started: Instant,
    /// Set when the user turned HDR off while this trigger ran; it then no
    /// longer turns HDR on until it ends.
    suppressed: bool,
}

#[derive(Debug)]
struct Slot {
    phase: Phase,
    observed: HdrState,
    /// The trigger that last kept this display in HDR: its hold time and
    /// name apply when HDR turns off.
    holder: Option<Holder>,
}

#[derive(Debug, Clone)]
struct Holder {
    trigger: TriggerId,
    label: String,
    hold: Duration,
}

/// The decision engine. See the module documentation.
#[derive(Debug)]
pub struct Engine {
    mode: Mode,
    paused_until: Option<Instant>,
    default_hold: Duration,
    slots: BTreeMap<DisplayId, Slot>,
    active: BTreeMap<TriggerId, Active>,
    /// The displays Always HDR mode and the shortcut turn on.
    manual: Target,
}

impl Engine {
    /// Starts with the managed displays as they are now. Displays in `owned`
    /// (from the recovery record) that are in HDR are treated as turned on by
    /// Huedra, so they return to SDR after `default_hold` unless a trigger
    /// starts; every other display in HDR is left alone. `manual` is the set
    /// of displays Always HDR mode and the shortcut turn on.
    pub fn new(
        mode: Mode,
        default_hold: Duration,
        displays: &[(DisplayId, HdrState)],
        owned: &[DisplayId],
        manual: Target,
        now: Instant,
    ) -> (Self, Outcome) {
        let slots = displays
            .iter()
            .map(|(id, state)| {
                let phase = match state {
                    HdrState::Off => Phase::Sdr,
                    HdrState::On if owned.contains(id) => Phase::HdrByHuedra,
                    HdrState::On => Phase::HdrExternal,
                };
                (
                    id.clone(),
                    Slot {
                        phase,
                        observed: *state,
                        holder: None,
                    },
                )
            })
            .collect();
        let mut engine = Self {
            mode,
            paused_until: None,
            default_hold,
            slots,
            active: BTreeMap::new(),
            manual,
        };
        let mut outcome = Outcome::default();
        engine.evaluate(now, &mut outcome);
        (engine, outcome)
    }

    /// Current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Changes the displays Always HDR mode and the shortcut turn on. A
    /// display it no longer covers keeps its state and follows Automatic
    /// mode from now on.
    pub fn set_manual_target(&mut self, now: Instant, manual: Target) -> Outcome {
        if self.manual == manual {
            return Outcome::default();
        }
        self.manual = manual;
        self.run(now)
    }

    /// Whether Always HDR mode and the shortcut turn this display on. A
    /// target that covers none of the managed displays (its display was
    /// disconnected) falls back to all of them, so the two never do nothing.
    fn manual_covers(&self, id: &DisplayId) -> bool {
        self.manual.covers(id) || !self.slots.keys().any(|d| self.manual.covers(d))
    }

    /// Changes the hold time used when no trigger supplied one.
    pub fn set_default_hold(&mut self, hold: Duration) {
        self.default_hold = hold;
    }

    /// Replaces the set of managed displays, for example after a monitor was
    /// connected or the settings changed. Known displays keep their phase.
    pub fn set_displays(&mut self, now: Instant, displays: &[(DisplayId, HdrState)]) -> Outcome {
        let mut outcome = Outcome::default();
        self.slots
            .retain(|id, _| displays.iter().any(|(d, _)| d == id));
        for (id, state) in displays {
            if self.slots.contains_key(id) {
                self.observe_one(id, *state, &mut outcome);
            } else {
                let phase = match state {
                    HdrState::Off => Phase::Sdr,
                    HdrState::On => Phase::HdrExternal,
                };
                self.slots.insert(
                    id.clone(),
                    Slot {
                        phase,
                        observed: *state,
                        holder: None,
                    },
                );
            }
        }
        self.evaluate(now, &mut outcome);
        outcome
    }

    /// Reports the state Windows shows for a display.
    pub fn observe(&mut self, now: Instant, display: &DisplayId, state: HdrState) -> Outcome {
        let mut outcome = Outcome::default();
        self.observe_one(display, state, &mut outcome);
        self.evaluate(now, &mut outcome);
        outcome
    }

    /// A trigger started (a game or video began).
    pub fn trigger_started(&mut self, now: Instant, id: TriggerId, trigger: Trigger) -> Outcome {
        self.active.entry(id).or_insert(Active {
            trigger,
            started: now,
            suppressed: false,
        });
        self.run(now)
    }

    /// A trigger ended.
    pub fn trigger_ended(&mut self, now: Instant, id: &TriggerId) -> Outcome {
        self.active.remove(id);
        self.run(now)
    }

    /// Changes the displays a running trigger wants in HDR, for example
    /// after its window moved to another display. A display it leaves
    /// holds HDR for the trigger's hold time, as if it had ended there.
    pub fn retarget(&mut self, now: Instant, id: &TriggerId, target: Target) -> Outcome {
        match self.active.get_mut(id) {
            Some(active) if active.trigger.target != target => {
                active.trigger.target = target;
                self.run(now)
            }
            _ => Outcome::default(),
        }
    }

    /// Changes the mode.
    pub fn set_mode(&mut self, now: Instant, mode: Mode) -> Outcome {
        self.mode = mode;
        self.run(now)
    }

    /// Stops all automatic switching until `until`; the current state is kept.
    pub fn pause(&mut self, now: Instant, until: Instant) -> Outcome {
        self.paused_until = Some(until);
        self.run(now)
    }

    /// Ends a pause early.
    pub fn resume(&mut self, now: Instant) -> Outcome {
        self.paused_until = None;
        self.run(now)
    }

    /// Whether switching is paused at `now`.
    pub fn is_paused(&self, now: Instant) -> bool {
        self.paused_until.is_some_and(|until| now < until)
    }

    /// The "Turn HDR on or off" shortcut. Turning HDR off also silences the
    /// running triggers; HDR turned on this way is the user's and is not
    /// turned off automatically. A mode that would undo the change is
    /// replaced by Automatic.
    pub fn toggle(&mut self, now: Instant) -> Outcome {
        let mut outcome = Outcome::default();
        let any_on = self.slots.values().any(|s| s.observed == HdrState::On);
        if any_on {
            if self.mode == Mode::AlwaysHdr {
                self.mode = Mode::Automatic;
            }
            self.suppress_all();
            for (id, slot) in &mut self.slots {
                if slot.observed == HdrState::On {
                    slot.phase = Phase::Sdr;
                    switch(
                        &mut outcome,
                        id,
                        slot,
                        HdrState::Off,
                        "Turned off with the shortcut",
                    );
                }
            }
        } else {
            if self.mode == Mode::AlwaysSdr {
                self.mode = Mode::Automatic;
            }
            let covered: Vec<DisplayId> = self
                .slots
                .keys()
                .filter(|d| self.manual_covers(d))
                .cloned()
                .collect();
            for (id, slot) in &mut self.slots {
                if !covered.contains(id) {
                    continue;
                }
                slot.phase = Phase::HdrExternal;
                switch(
                    &mut outcome,
                    id,
                    slot,
                    HdrState::On,
                    "Turned on with the shortcut",
                );
            }
        }
        self.evaluate(now, &mut outcome);
        outcome
    }

    /// Windows did not apply an action.
    pub fn switch_failed(
        &mut self,
        now: Instant,
        display: &DisplayId,
        wanted: HdrState,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        if let Some(slot) = self.slots.get_mut(display) {
            match wanted {
                HdrState::On => {
                    slot.phase = Phase::Sdr;
                    slot.observed = HdrState::Off;
                    self.suppress_covering(display);
                }
                HdrState::Off => {
                    slot.phase = Phase::Holding {
                        until: now + OFF_RETRY,
                    };
                    slot.observed = HdrState::On;
                }
            }
            outcome.notes.push(Note {
                display: display.clone(),
                kind: NoteKind::Failed,
                reason: "Windows didn't switch the display".to_owned(),
            });
        }
        self.evaluate(now, &mut outcome);
        outcome
    }

    /// Call when [`Self::next_deadline`] has passed.
    pub fn tick(&mut self, now: Instant) -> Outcome {
        self.run(now)
    }

    /// The next time the engine needs a [`Self::tick`], if any. `None` means
    /// the caller can sleep until the next event.
    pub fn next_deadline(&self) -> Option<Instant> {
        let phases = self.slots.values().filter_map(|s| match s.phase {
            Phase::Waiting { until } | Phase::Holding { until } => Some(until),
            _ => None,
        });
        phases.chain(self.paused_until).min()
    }

    /// The label of a running trigger, also one set never to switch HDR.
    pub fn label(&self, id: &TriggerId) -> Option<&str> {
        self.active.get(id).map(|a| a.trigger.label.as_str())
    }

    /// Running triggers that still want HDR, oldest first, as (id, label).
    pub fn active_triggers(&self) -> Vec<(TriggerId, String)> {
        let mut live: Vec<(&TriggerId, &Active)> =
            self.active.iter().filter(|(_, a)| !a.suppressed).collect();
        live.sort_by_key(|(_, a)| a.started);
        live.into_iter()
            .map(|(id, a)| (id.clone(), a.trigger.label.clone()))
            .collect()
    }

    /// Displays whose HDR belongs to Huedra, for the recovery record.
    pub fn owned(&self) -> Vec<DisplayId> {
        self.slots
            .iter()
            .filter(|(_, s)| s.phase.is_owned())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// A summary for the tray and the settings window.
    pub fn status(&self, now: Instant) -> Status {
        let reason = self
            .active
            .values()
            .filter(|a| !a.suppressed)
            .min_by_key(|a| a.started)
            .map(|a| a.trigger.label.clone());
        Status {
            mode: self.mode,
            paused_until: self.paused_until.filter(|&until| now < until),
            hdr_on: self.slots.values().any(|s| s.observed == HdrState::On),
            reason,
            displays: self
                .slots
                .iter()
                .map(|(id, s)| (id.clone(), s.phase))
                .collect(),
        }
    }

    fn run(&mut self, now: Instant) -> Outcome {
        let mut outcome = Outcome::default();
        self.evaluate(now, &mut outcome);
        outcome
    }

    fn suppress_all(&mut self) {
        for active in self.active.values_mut() {
            active.suppressed = true;
        }
    }

    /// Silences the triggers that want this display in HDR.
    fn suppress_covering(&mut self, display: &DisplayId) {
        for active in self.active.values_mut() {
            if active.trigger.target.covers(display) {
                active.suppressed = true;
            }
        }
    }

    fn observe_one(&mut self, display: &DisplayId, state: HdrState, outcome: &mut Outcome) {
        let Some(slot) = self.slots.get_mut(display) else {
            return;
        };
        let previous = slot.observed;
        slot.observed = state;
        if previous == state {
            return;
        }
        let note = |kind, reason: &str| Note {
            display: display.clone(),
            kind,
            reason: reason.to_owned(),
        };
        match (slot.phase, state) {
            (Phase::Sdr | Phase::Waiting { .. }, HdrState::On) => {
                slot.phase = Phase::HdrExternal;
                outcome.notes.push(note(
                    NoteKind::External,
                    "HDR was already on, so Huedra will leave it on",
                ));
            }
            (Phase::HdrByHuedra | Phase::Holding { .. } | Phase::HdrExternal, HdrState::Off) => {
                slot.phase = Phase::Sdr;
                outcome.notes.push(note(
                    NoteKind::External,
                    "HDR was turned off outside Huedra",
                ));
                self.suppress_covering(display);
            }
            _ => {}
        }
    }

    fn evaluate(&mut self, now: Instant, outcome: &mut Outcome) {
        if let Some(until) = self.paused_until {
            if now < until {
                return;
            }
            self.paused_until = None;
        }
        let mode = self.mode;
        let manual: BTreeSet<DisplayId> = self
            .slots
            .keys()
            .filter(|d| self.manual_covers(d))
            .cloned()
            .collect();
        let default_hold = self.default_hold;
        let active = &self.active;
        for (id, slot) in &mut self.slots {
            // Only the triggers that want this display count for it.
            let live = || {
                active
                    .iter()
                    .filter(|(_, a)| !a.suppressed && a.trigger.target.covers(id))
            };
            let first_ready = live()
                .filter(|(_, a)| now >= a.started + a.trigger.delay)
                .min_by_key(|(_, a)| a.started);
            if let Some((trigger, a)) = first_ready {
                slot.holder = Some(Holder {
                    trigger: trigger.clone(),
                    label: a.trigger.label.clone(),
                    hold: a.trigger.hold,
                });
            }
            let ready = first_ready.map(|(_, a)| a.trigger.label.clone());
            let pending = live().map(|(_, a)| a.started + a.trigger.delay).min();
            let hold = slot.holder.as_ref().map_or(default_hold, |h| h.hold);
            let off_reason = closed_reason(slot.holder.as_ref(), active, hold);
            // Always HDR covers only the manual target; the other displays
            // keep following their games and video.
            let mode = match mode {
                Mode::AlwaysHdr if !manual.contains(id) => Mode::Automatic,
                mode => mode,
            };
            match mode {
                Mode::AlwaysHdr => match slot.phase {
                    Phase::Sdr | Phase::Waiting { .. } => {
                        slot.phase = Phase::HdrByHuedra;
                        switch(outcome, id, slot, HdrState::On, "Always HDR mode");
                    }
                    Phase::Holding { .. } => slot.phase = Phase::HdrByHuedra,
                    Phase::HdrByHuedra | Phase::HdrExternal => {}
                },
                Mode::AlwaysSdr => match slot.phase {
                    Phase::Waiting { .. } => slot.phase = Phase::Sdr,
                    Phase::HdrByHuedra | Phase::Holding { .. } => {
                        slot.phase = Phase::Sdr;
                        switch(outcome, id, slot, HdrState::Off, "Always SDR mode");
                    }
                    Phase::Sdr | Phase::HdrExternal => {}
                },
                Mode::Automatic => match (slot.phase, &ready, pending) {
                    (Phase::HdrExternal, _, _) => {}
                    (Phase::Sdr | Phase::Waiting { .. }, Some(label), _) => {
                        slot.phase = Phase::HdrByHuedra;
                        let reason = format!("{label} is running");
                        switch(outcome, id, slot, HdrState::On, &reason);
                    }
                    (Phase::Sdr | Phase::Waiting { .. }, None, Some(until)) => {
                        slot.phase = Phase::Waiting { until };
                    }
                    (Phase::Waiting { .. }, None, None) => slot.phase = Phase::Sdr,
                    (Phase::Sdr, None, None) => {}
                    (Phase::HdrByHuedra | Phase::Holding { .. }, Some(_), _)
                    | (Phase::Holding { .. }, None, Some(_)) => slot.phase = Phase::HdrByHuedra,
                    (Phase::HdrByHuedra, None, Some(_)) => {}
                    (Phase::HdrByHuedra, None, None) if hold.is_zero() => {
                        slot.phase = Phase::Sdr;
                        slot.holder = None;
                        switch(outcome, id, slot, HdrState::Off, &off_reason);
                    }
                    (Phase::HdrByHuedra, None, None) => {
                        slot.phase = Phase::Holding { until: now + hold };
                    }
                    (Phase::Holding { until }, None, None) => {
                        if now >= until {
                            slot.phase = Phase::Sdr;
                            slot.holder = None;
                            switch(outcome, id, slot, HdrState::Off, &off_reason);
                        }
                    }
                },
            }
        }
    }
}

/// Records an action and its note, and updates the expected state.
fn switch(outcome: &mut Outcome, id: &DisplayId, slot: &mut Slot, state: HdrState, reason: &str) {
    if slot.observed == state {
        return;
    }
    slot.observed = state;
    outcome.actions.push(Action {
        display: id.clone(),
        state,
    });
    outcome.notes.push(Note {
        display: id.clone(),
        kind: match state {
            HdrState::On => NoteKind::Enabled,
            HdrState::Off => NoteKind::Disabled,
        },
        reason: reason.to_owned(),
    });
}

/// Why a display returns to SDR: its last trigger closed, or moved to
/// another display while still running.
fn closed_reason(
    holder: Option<&Holder>,
    active: &BTreeMap<TriggerId, Active>,
    hold: Duration,
) -> String {
    let secs = hold.as_secs();
    let unit = if secs == 1 { "second" } else { "seconds" };
    let kept = if secs == 0 {
        String::new()
    } else {
        format!(" (HDR kept on {secs} {unit} longer)")
    };
    match holder {
        None => format!("Nothing needs HDR now{kept}"),
        Some(h) if active.contains_key(&h.trigger) => {
            format!("{} moved to another display{kept}", h.label)
        }
        Some(h) => format!("{} closed{kept}", h.label),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLD: Duration = Duration::from_secs(10);

    fn display() -> DisplayId {
        DisplayId::new("monitor-1")
    }

    fn game(delay: u64) -> Trigger {
        Trigger {
            label: "Cyberpunk 2077".into(),
            delay: Duration::from_secs(delay),
            hold: HOLD,
            target: Target::All,
        }
    }

    fn engine(state: HdrState, owned: bool) -> (Engine, Instant) {
        let now = Instant::now();
        let owned = if owned { vec![display()] } else { vec![] };
        let (engine, _) = Engine::new(
            Mode::Automatic,
            HOLD,
            &[(display(), state)],
            &owned,
            Target::All,
            now,
        );
        (engine, now)
    }

    fn on() -> Vec<Action> {
        vec![Action {
            display: display(),
            state: HdrState::On,
        }]
    }

    fn off() -> Vec<Action> {
        vec![Action {
            display: display(),
            state: HdrState::Off,
        }]
    }

    fn phase(engine: &Engine) -> Phase {
        engine.slots[&display()].phase
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn trigger_without_delay_turns_hdr_on_then_holds_then_off() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let id = TriggerId::new("steam:1");
        let out = e.trigger_started(t0, id.clone(), game(0));
        assert_eq!(out.actions, on(), "on at start");
        assert_eq!(
            out.notes[0].reason, "Cyberpunk 2077 is running",
            "reason names the game"
        );
        assert_eq!(phase(&e), Phase::HdrByHuedra, "owned");

        let out = e.trigger_ended(t0 + secs(60), &id);
        assert!(out.actions.is_empty(), "held, not off yet");
        assert_eq!(e.next_deadline(), Some(t0 + secs(70)), "timer for hold");

        assert!(e.tick(t0 + secs(69)).actions.is_empty(), "still holding");
        let out = e.tick(t0 + secs(70));
        assert_eq!(out.actions, off(), "off after hold");
        assert_eq!(
            out.notes[0].reason, "Cyberpunk 2077 closed (HDR kept on 10 seconds longer)",
            "reason"
        );
        assert_eq!(e.next_deadline(), None, "idle");
    }

    #[test]
    fn label_names_a_running_trigger_only() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let id = TriggerId::new("steam:1");
        assert_eq!(e.label(&id), None, "not started");
        e.trigger_started(t0, id.clone(), game(0));
        assert_eq!(e.label(&id), Some("Cyberpunk 2077"), "running");
        e.trigger_ended(t0 + secs(1), &id);
        assert_eq!(e.label(&id), None, "ended");
    }

    #[test]
    fn delay_waits_and_cancels() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let id = TriggerId::new("a");
        assert!(
            e.trigger_started(t0, id.clone(), game(5))
                .actions
                .is_empty(),
            "waits"
        );
        assert_eq!(
            phase(&e),
            Phase::Waiting {
                until: t0 + secs(5)
            },
            "waiting"
        );
        assert!(
            e.trigger_ended(t0 + secs(2), &id).actions.is_empty(),
            "no switch"
        );
        assert_eq!(phase(&e), Phase::Sdr, "back to SDR");

        e.trigger_started(t0 + secs(10), id, game(5));
        assert_eq!(e.tick(t0 + secs(15)).actions, on(), "on when delay elapsed");
    }

    #[test]
    fn new_trigger_during_hold_keeps_hdr() {
        let (mut e, t0) = engine(HdrState::Off, false);
        e.trigger_started(t0, TriggerId::new("a"), game(0));
        e.trigger_ended(t0 + secs(1), &TriggerId::new("a"));
        let out = e.trigger_started(t0 + secs(5), TriggerId::new("b"), game(3));
        assert!(out.actions.is_empty(), "no flicker");
        assert_eq!(phase(&e), Phase::HdrByHuedra, "back to owned");
        assert!(e.tick(t0 + secs(30)).actions.is_empty(), "stays on");
    }

    #[test]
    fn external_hdr_is_never_turned_off() {
        let (mut e, t0) = engine(HdrState::On, false);
        assert_eq!(phase(&e), Phase::HdrExternal, "external at start");
        let id = TriggerId::new("a");
        assert!(
            e.trigger_started(t0, id.clone(), game(0))
                .actions
                .is_empty(),
            "already on"
        );
        assert!(
            e.trigger_ended(t0, &id).actions.is_empty(),
            "not turned off"
        );
        assert!(e.tick(t0 + secs(3600)).actions.is_empty(), "ever");
        assert!(
            e.set_mode(t0, Mode::AlwaysSdr).actions.is_empty(),
            "not even by Always SDR"
        );
    }

    #[test]
    fn enabled_outside_while_sdr_becomes_external() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let out = e.observe(t0, &display(), HdrState::On);
        assert_eq!(out.notes[0].kind, NoteKind::External, "noted");
        assert_eq!(phase(&e), Phase::HdrExternal, "external");
        e.observe(t0, &display(), HdrState::Off);
        assert_eq!(phase(&e), Phase::Sdr, "disabled outside returns to SDR");
    }

    #[test]
    fn turned_off_outside_during_game_is_respected() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let id = TriggerId::new("a");
        e.trigger_started(t0, id.clone(), game(0));
        let out = e.observe(t0 + secs(5), &display(), HdrState::Off);
        assert!(out.actions.is_empty(), "not turned back on");
        assert!(e.tick(t0 + secs(60)).actions.is_empty(), "stays off");
        e.trigger_ended(t0 + secs(70), &id);
        assert_eq!(
            e.trigger_started(t0 + secs(80), id, game(0)).actions,
            on(),
            "next start works again"
        );
    }

    #[test]
    fn recovery_turns_off_owned_hdr_after_hold() {
        let (mut e, t0) = engine(HdrState::On, true);
        assert_eq!(phase(&e), Phase::Holding { until: t0 + HOLD }, "holding");
        let out = e.tick(t0 + HOLD);
        assert_eq!(out.actions, off(), "restored");
        assert_eq!(
            out.notes[0].reason, "Nothing needs HDR now (HDR kept on 10 seconds longer)",
            "reason"
        );
    }

    #[test]
    fn modes() {
        let (mut e, t0) = engine(HdrState::Off, false);
        assert_eq!(e.set_mode(t0, Mode::AlwaysHdr).actions, on(), "always HDR");
        assert_eq!(e.owned(), vec![display()], "owned");
        assert!(
            e.set_mode(t0, Mode::Automatic).actions.is_empty(),
            "held after leaving"
        );
        assert_eq!(e.tick(t0 + HOLD).actions, off(), "then off");
        let id = TriggerId::new("a");
        e.set_mode(t0, Mode::AlwaysSdr);
        assert!(
            e.trigger_started(t0, id, game(0)).actions.is_empty(),
            "always SDR ignores triggers"
        );
        assert_eq!(e.set_mode(t0, Mode::Automatic).actions, on(), "resumes");
        assert_eq!(Mode::AlwaysSdr.next(), Mode::Automatic, "cycle wraps");
    }

    #[test]
    fn pause_keeps_state() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let id = TriggerId::new("a");
        e.pause(t0, t0 + secs(3600));
        assert!(e.is_paused(t0), "paused");
        assert!(
            e.trigger_started(t0, id.clone(), game(0))
                .actions
                .is_empty(),
            "no switching"
        );
        assert_eq!(e.next_deadline(), Some(t0 + secs(3600)), "wakes at end");
        assert_eq!(e.tick(t0 + secs(3600)).actions, on(), "evaluates at end");
        e.pause(t0 + secs(3601), t0 + secs(7200));
        assert!(
            e.trigger_ended(t0 + secs(3602), &id).actions.is_empty(),
            "HDR kept"
        );
        e.resume(t0 + secs(3603));
        assert!(!e.is_paused(t0 + secs(3603)), "resumed");
        assert_eq!(e.tick(t0 + secs(3613)).actions, off(), "then normal");
    }

    #[test]
    fn toggle_is_the_users_choice() {
        let (mut e, t0) = engine(HdrState::Off, false);
        assert_eq!(e.toggle(t0).actions, on(), "on");
        assert_eq!(phase(&e), Phase::HdrExternal, "never auto-off");
        assert!(e.owned().is_empty(), "not recorded as Huedra's");
        assert_eq!(e.toggle(t0).actions, off(), "off");

        let id = TriggerId::new("a");
        e.trigger_started(t0, id, game(0));
        assert_eq!(e.toggle(t0).actions, off(), "off during game");
        assert!(e.tick(t0 + secs(5)).actions.is_empty(), "game silenced");

        e.set_mode(t0, Mode::AlwaysHdr);
        e.toggle(t0);
        assert_eq!(e.mode(), Mode::Automatic, "Always HDR gives way");
    }

    #[test]
    fn manual_target_limits_always_hdr_and_the_shortcut() {
        let t0 = Instant::now();
        let (one, two) = (display(), DisplayId::new("monitor-2"));
        let both = [(one.clone(), HdrState::Off), (two.clone(), HdrState::Off)];
        let main = Target::Only(BTreeSet::from([one.clone()]));
        let (mut e, _) = Engine::new(Mode::Automatic, HOLD, &both, &[], main, t0);
        let only = |d: &DisplayId, state| {
            vec![Action {
                display: d.clone(),
                state,
            }]
        };

        assert_eq!(
            e.toggle(t0).actions,
            only(&one, HdrState::On),
            "shortcut: main only"
        );
        assert_eq!(
            e.toggle(t0).actions,
            only(&one, HdrState::Off),
            "shortcut off"
        );
        assert_eq!(
            e.set_mode(t0, Mode::AlwaysHdr).actions,
            only(&one, HdrState::On),
            "Always HDR: main only"
        );

        // The other display still follows a game shown on it.
        let mut on_two = game(0);
        on_two.target = Target::Only(BTreeSet::from([two.clone()]));
        let out = e.trigger_started(t0, TriggerId::new("a"), on_two);
        assert_eq!(out.actions, only(&two, HdrState::On), "game on display 2");

        // Every display.
        let (mut e, _) = Engine::new(Mode::Automatic, HOLD, &both, &[], Target::All, t0);
        assert_eq!(e.toggle(t0).actions.len(), 2, "shortcut: all");
        e.toggle(t0);
        let out = e.set_manual_target(t0, Target::Only(BTreeSet::from([two.clone()])));
        assert!(
            out.actions.is_empty(),
            "changing the setting switches nothing"
        );
        assert_eq!(
            e.toggle(t0).actions,
            only(&two, HdrState::On),
            "now display 2"
        );

        // A main display that is gone falls back to every display.
        let gone = Target::Only(BTreeSet::from([DisplayId::new("gone")]));
        let (_, out) = Engine::new(Mode::AlwaysHdr, HOLD, &both, &[], gone.clone(), t0);
        assert_eq!(out.actions.len(), 2, "fallback: Always HDR on all");
        let (mut e, _) = Engine::new(Mode::Automatic, HOLD, &both, &[], gone, t0);
        assert_eq!(e.toggle(t0).actions.len(), 2, "fallback: shortcut on all");
    }

    #[test]
    fn failed_switch_does_not_loop() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let id = TriggerId::new("a");
        e.trigger_started(t0, id, game(0));
        let out = e.switch_failed(t0, &display(), HdrState::On);
        assert!(out.actions.is_empty(), "no immediate retry");
        assert_eq!(out.notes[0].kind, NoteKind::Failed, "reported");
        assert!(e.tick(t0 + secs(60)).actions.is_empty(), "no retry loop");
    }

    #[test]
    fn displays_can_change() {
        let (mut e, t0) = engine(HdrState::Off, false);
        let second = DisplayId::new("monitor-2");
        e.set_displays(
            t0,
            &[(display(), HdrState::Off), (second.clone(), HdrState::On)],
        );
        assert_eq!(e.status(t0).displays.len(), 2, "added");
        assert_eq!(
            e.status(t0).displays[1],
            (second, Phase::HdrExternal),
            "new display in HDR is external"
        );
        e.set_displays(t0, &[]);
        assert!(e.status(t0).displays.is_empty(), "removed");
    }

    #[test]
    fn target_switches_only_its_displays() {
        let t0 = Instant::now();
        let (one, two) = (display(), DisplayId::new("monitor-2"));
        let both = [(one.clone(), HdrState::Off), (two.clone(), HdrState::Off)];
        let (mut e, _) = Engine::new(Mode::Automatic, HOLD, &both, &[], Target::All, t0);
        let only = |d: &DisplayId| Target::Only(BTreeSet::from([d.clone()]));
        let id = TriggerId::new("video:chrome:1");

        // Window not known yet: nothing switches.
        let mut video = game(0);
        video.target = Target::Only(BTreeSet::new());
        let out = e.trigger_started(t0, id.clone(), video);
        assert!(out.actions.is_empty(), "no window");

        let out = e.retarget(t0, &id, only(&two));
        let on_two = Action {
            display: two.clone(),
            state: HdrState::On,
        };
        assert_eq!(out.actions, vec![on_two], "only the window's display");
        assert!(
            e.retarget(t0, &id, only(&two)).actions.is_empty(),
            "same target"
        );

        // The window moves: the new display turns on, the old one holds.
        let out = e.retarget(t0 + secs(1), &id, only(&one));
        assert_eq!(out.actions, on(), "new display on");
        let phases = e.status(t0).displays;
        let until = t0 + secs(1) + HOLD;
        assert_eq!(phases[1].1, Phase::Holding { until }, "old holds");
        let out = e.tick(until);
        assert_eq!(out.actions.len(), 1, "old display off");
        assert_eq!(out.actions[0].display, two, "old display");
        assert_eq!(
            out.notes[0].reason,
            "Cyberpunk 2077 moved to another display (HDR kept on 10 seconds longer)",
            "reason"
        );

        // Turning HDR off on one display silences only its triggers.
        let other = TriggerId::new("video:mpv:2");
        let mut player = game(0);
        player.target = only(&two);
        e.trigger_started(t0 + secs(20), other.clone(), player);
        e.observe(t0 + secs(21), &one, HdrState::Off);
        let running = e.active_triggers();
        assert_eq!(running.len(), 1, "one trigger silenced");
        assert_eq!(running[0].0, other, "the other display's trigger runs");
    }

    #[test]
    fn status_names_the_trigger() {
        let (mut e, t0) = engine(HdrState::Off, false);
        e.trigger_started(t0, TriggerId::new("a"), game(0));
        let status = e.status(t0);
        assert!(status.hdr_on, "on");
        assert_eq!(status.reason.as_deref(), Some("Cyberpunk 2077"), "reason");
    }
}
