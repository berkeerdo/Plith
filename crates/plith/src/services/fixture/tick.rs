//! The fixture ticker: a thread that asks the fixture to republish its readings every
//! [`TICK`], so `--idle-check` with `--fixture` covers a service publishing at rest.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The fixture's poll period at rest.
pub const TICK: Duration = Duration::from_millis(250);

/// The ticker thread; stopped and joined by [`Ticker::stop`] or on drop.
pub struct Ticker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Ticker {
    /// Sends a tick to `ticks` and calls `wake` every `period`, until stopped or until the
    /// receiver is gone.
    ///
    /// # Errors
    /// When the thread cannot be spawned.
    pub fn spawn(
        period: Duration,
        ticks: Sender<()>,
        wake: Box<dyn Fn() + Send>,
    ) -> std::io::Result<Ticker> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("plith-fixture-tick".into())
            .spawn(move || {
                loop {
                    thread::sleep(period);
                    if stopped.load(Ordering::Acquire) || ticks.send(()).is_err() {
                        return;
                    }
                    wake();
                }
            })?;
        Ok(Ticker {
            stop,
            thread: Some(thread),
        })
    }

    /// Stops the thread and waits for it (at most one period).
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.halt();
    }
}
