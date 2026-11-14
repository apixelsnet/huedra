//! Switches HDR on a worker thread, so the tray's message loop keeps
//! answering the menu, the shortcuts and the command line while Windows
//! changes a display (a few hundred milliseconds each, seconds when a
//! display refuses). Feature doc: `docs/features/01-hdr-switch.md`.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use huedra_core::display::{DisplayId, HdrState};

use crate::display::{self, Switched};
use crate::event_loop::{WM_SWITCHED, Waker};

/// One request: the changes and the number of tries for each display.
#[derive(Debug)]
struct Batch {
    changes: Vec<(DisplayId, HdrState)>,
    attempts: u32,
}

/// The worker and the changes it has not finished.
#[derive(Debug)]
pub struct Switcher {
    jobs: mpsc::Sender<Batch>,
    results: mpsc::Receiver<Vec<Switched>>,
    in_flight: usize,
}

impl Switcher {
    /// Starts the worker. Each finished batch wakes the loop with
    /// [`crate::event_loop::Event::Switched`].
    ///
    /// # Errors
    /// When the thread cannot start.
    pub fn start(waker: Waker) -> std::io::Result<Self> {
        let (jobs, inbox) = mpsc::channel::<Batch>();
        let (outbox, results) = mpsc::channel();
        thread::Builder::new()
            .name("huedra-switch".into())
            .spawn(move || {
                // Batches run in the order they were asked for, so a later
                // "off" never overtakes an earlier "on".
                for batch in inbox {
                    let done = display::set_hdr_many(&batch.changes, batch.attempts);
                    if outbox.send(done).is_err() {
                        return;
                    }
                    waker.post(WM_SWITCHED, 0);
                }
            })?;
        Ok(Self {
            jobs,
            results,
            in_flight: 0,
        })
    }

    /// Queues changes for the worker.
    pub fn request(&mut self, changes: Vec<(DisplayId, HdrState)>, attempts: u32) {
        if changes.is_empty() {
            return;
        }
        if self.jobs.send(Batch { changes, attempts }).is_ok() {
            self.in_flight += 1;
        }
    }

    /// Whether every queued change has finished and been collected.
    pub fn is_idle(&self) -> bool {
        self.in_flight == 0
    }

    /// The changes finished since the last call.
    pub fn finished(&mut self) -> Vec<Switched> {
        let mut all = Vec::new();
        while let Ok(done) = self.results.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            all.extend(done);
        }
        all
    }

    /// Waits until every queued change has finished, for at most `timeout`,
    /// and returns them. Used on exit, before the displays go back to SDR.
    pub fn wait_idle(&mut self, timeout: Duration) -> Vec<Switched> {
        let deadline = Instant::now() + timeout;
        let mut all = self.finished();
        while self.in_flight > 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.results.recv_timeout(left) {
                Ok(done) => {
                    self.in_flight -= 1;
                    all.extend(done);
                }
                Err(_) => break,
            }
        }
        all
    }
}
