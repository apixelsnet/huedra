//! Follows the processes behind each trigger, including games that start
//! through a launcher which exits after a few seconds.
//! Feature doc: `docs/features/03-watchers.md`.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use crate::engine::TriggerId;
use crate::paths::NormPath;

/// A process that exits sooner than this after it was first seen is treated
/// as a launcher.
pub const QUICK_EXIT: Duration = Duration::from_secs(10);
/// How long to look for the real game after a launcher exits.
pub const FOLLOW_WINDOW: Duration = Duration::from_secs(45);

/// A Windows process id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid(pub u32);

/// What the end of a tracked process means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// The trigger has no process left.
    Ended(TriggerId),
    /// It looked like a launcher; the folder is watched for the real game.
    Following,
    /// Another process of the same trigger is still running.
    StillRunning,
    /// The process was not tracked.
    Unknown,
}

#[derive(Debug)]
struct Tracked {
    trigger: TriggerId,
    folder: NormPath,
    seen: Instant,
}

#[derive(Debug)]
struct Follow {
    folder: NormPath,
    until: Instant,
}

/// Process bookkeeping for the engine's triggers.
#[derive(Debug, Default)]
pub struct Tracker {
    processes: HashMap<Pid, Tracked>,
    follows: BTreeMap<TriggerId, Follow>,
}

impl Tracker {
    /// Whether a process is already tracked.
    pub fn is_tracked(&self, pid: Pid) -> bool {
        self.processes.contains_key(&pid)
    }

    /// The trigger a tracked process belongs to.
    pub fn trigger_of(&self, pid: Pid) -> Option<&TriggerId> {
        self.processes.get(&pid).map(|p| &p.trigger)
    }

    /// Whether a trigger has a process or a follow window.
    pub fn is_active(&self, trigger: &TriggerId) -> bool {
        self.follows.contains_key(trigger) || self.processes.values().any(|p| &p.trigger == trigger)
    }

    /// Starts tracking a process for `trigger`. `folder` is where a launcher's
    /// successor is looked for. Returns `true` when the trigger was not
    /// active before, so the engine must be told it started.
    pub fn track(&mut self, pid: Pid, trigger: TriggerId, folder: &str, now: Instant) -> bool {
        let new = !self.is_active(&trigger);
        self.follows.remove(&trigger);
        self.processes.insert(
            pid,
            Tracked {
                trigger,
                folder: NormPath::new(folder),
                seen: now,
            },
        );
        new
    }

    /// A tracked process exited.
    pub fn exited(&mut self, pid: Pid, now: Instant) -> Exit {
        let Some(gone) = self.processes.remove(&pid) else {
            return Exit::Unknown;
        };
        if self.processes.values().any(|p| p.trigger == gone.trigger) {
            return Exit::StillRunning;
        }
        if now.saturating_duration_since(gone.seen) < QUICK_EXIT {
            self.follows.insert(
                gone.trigger,
                Follow {
                    folder: gone.folder,
                    until: now + FOLLOW_WINDOW,
                },
            );
            return Exit::Following;
        }
        Exit::Ended(gone.trigger)
    }

    /// Offers a process found while following. Returns the trigger it was
    /// attached to, if its program lies in a followed folder.
    pub fn adopt(&mut self, pid: Pid, exe_path: &str, now: Instant) -> Option<TriggerId> {
        if self.is_tracked(pid) {
            return None;
        }
        let exe = NormPath::new(exe_path);
        let trigger = self
            .follows
            .iter()
            .find(|(_, f)| now < f.until && exe.is_within(&f.folder))
            .map(|(t, _)| t.clone())?;
        let folder = self.follows.remove(&trigger).map(|f| f.folder)?;
        self.processes.insert(
            pid,
            Tracked {
                trigger: trigger.clone(),
                folder,
                seen: now,
            },
        );
        Some(trigger)
    }

    /// Ends follow windows that ran out; returns their triggers.
    pub fn expire(&mut self, now: Instant) -> Vec<TriggerId> {
        let done: Vec<TriggerId> = self
            .follows
            .iter()
            .filter(|(_, f)| now >= f.until)
            .map(|(t, _)| t.clone())
            .collect();
        for trigger in &done {
            self.follows.remove(trigger);
        }
        done
    }

    /// Whether any follow window is open (the only time the process list is
    /// checked periodically).
    pub fn is_following(&self) -> bool {
        !self.follows.is_empty()
    }

    /// When the earliest follow window closes.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.follows.values().map(|f| f.until).min()
    }

    /// Process ids being tracked.
    pub fn pids(&self) -> impl Iterator<Item = Pid> + '_ {
        self.processes.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"D:\Games\Foo";

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn long_running_process_ends_trigger() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        let id = TriggerId::new("foo");
        assert!(t.track(Pid(1), id.clone(), DIR, t0), "new trigger");
        assert!(!t.track(Pid(2), id.clone(), DIR, t0), "second process");
        assert_eq!(
            t.exited(Pid(1), t0 + secs(60)),
            Exit::StillRunning,
            "one left"
        );
        assert_eq!(t.exited(Pid(2), t0 + secs(60)), Exit::Ended(id), "ended");
        assert_eq!(t.exited(Pid(9), t0), Exit::Unknown, "unknown");
    }

    #[test]
    fn launcher_is_followed() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        let id = TriggerId::new("foo");
        t.track(Pid(1), id.clone(), DIR, t0);
        assert_eq!(
            t.exited(Pid(1), t0 + secs(3)),
            Exit::Following,
            "quick exit"
        );
        assert!(t.is_active(&id), "still active while following");
        assert_eq!(t.next_deadline(), Some(t0 + secs(48)), "window");
        assert_eq!(
            t.adopt(Pid(2), r"C:\Other\x.exe", t0 + secs(5)),
            None,
            "other folder"
        );
        assert_eq!(
            t.adopt(Pid(3), r"d:\games\foo\bin\Foo.exe", t0 + secs(6)),
            Some(id.clone()),
            "successor"
        );
        assert!(!t.is_following(), "window closed");
        assert_eq!(
            t.exited(Pid(3), t0 + secs(600)),
            Exit::Ended(id),
            "then normal"
        );
    }

    #[test]
    fn follow_window_expires() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        let id = TriggerId::new("foo");
        t.track(Pid(1), id.clone(), DIR, t0);
        t.exited(Pid(1), t0 + secs(1));
        assert!(t.expire(t0 + secs(30)).is_empty(), "not yet");
        assert_eq!(t.expire(t0 + secs(46)), vec![id.clone()], "expired");
        assert!(!t.is_active(&id), "gone");
    }
}
