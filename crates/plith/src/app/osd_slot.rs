//! The OSD side as a setting: on, off and a new flyout mode while the app runs. Stopping a side
//! waits up to [`STOP_WAIT`] for its supervisor, so it happens on a short thread that wakes the
//! UI when done; a start asked for meanwhile waits for that stop (same pipe, same helper).

use crate::log::LogFile;
use crate::osd::{OsdSide, OsdSink, STOP_WAIT, join_within};
use plith_core::launch::FlyoutMode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

/// Starts one OSD side. A trait so tests can use a fake.
pub trait OsdStart {
    /// The running side; dropping it stops it, which may take a while.
    type Side: Send + 'static;
    /// Starts a side in `mode`; `None` when it cannot run.
    fn start(&self, mode: FlyoutMode) -> Option<Self::Side>;
    /// The sink the feed sends levels to.
    fn sink(side: &Self::Side) -> OsdSink;
}

/// Posts a wake to the UI thread; called from the stopping thread.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// The real side: the helper link and its supervisor.
pub struct RealOsd {
    /// The run log the side writes to.
    pub log: Arc<LogFile>,
}

impl OsdStart for RealOsd {
    type Side = OsdSide;

    fn start(&self, mode: FlyoutMode) -> Option<OsdSide> {
        OsdSide::start(mode, Arc::clone(&self.log))
    }

    fn sink(side: &OsdSide) -> OsdSink {
        side.sink()
    }
}

/// Where the slot is.
enum State<Side> {
    Off,
    Running(Side),
    /// The old side drops on `join`; `done` is set right before its wake. `then_start` starts
    /// a new side once it is done.
    Stopping {
        join: JoinHandle<()>,
        done: Arc<AtomicBool>,
        then_start: bool,
    },
}

/// The OSD side of the run, switched on the UI thread without blocking it.
pub struct OsdSlot<S: OsdStart> {
    starter: S,
    state: State<S::Side>,
    /// The mode the next start uses.
    mode: FlyoutMode,
    wake: Wake,
    /// The sink of the side started last, until the app hands it to the feed.
    sink: Option<OsdSink>,
}

impl<S: OsdStart> OsdSlot<S> {
    /// An empty slot; `wake` runs on the stopping thread when a stop finished.
    #[must_use]
    pub fn new(starter: S, wake: Wake) -> Self {
        Self {
            starter,
            state: State::Off,
            mode: FlyoutMode::Off,
            wake,
            sink: None,
        }
    }

    /// True when a side runs or starts once the old one stopped.
    #[must_use]
    pub fn is_on(&self) -> bool {
        match &self.state {
            State::Off => false,
            State::Running(_) => true,
            State::Stopping { then_start, .. } => *then_start,
        }
    }

    /// Turns the side on or off; `mode` is used by the next start. A side already running keeps
    /// its mode (see [`Self::restart`]). Never waits for a stop.
    pub fn set(&mut self, on: bool, mode: FlyoutMode) {
        self.mode = mode;
        match &mut self.state {
            State::Off if on => self.start(),
            State::Running(_) if !on => self.stop(false),
            State::Stopping { then_start, .. } => *then_start = on,
            _ => {}
        }
    }

    /// Stops a running side and starts it again in `mode` once it stopped. Off, it only keeps
    /// `mode` for the next start; while stopping, a pending start takes `mode`.
    pub fn restart(&mut self, mode: FlyoutMode) {
        self.mode = mode;
        if matches!(self.state, State::Running(_)) {
            self.stop(true);
        }
    }

    /// The settings' wish: on or off, and a restart when on in a new mode.
    pub fn apply(&mut self, on: bool, mode: FlyoutMode) {
        if on && self.is_on() && mode != self.mode {
            self.restart(mode);
        } else {
            self.set(on, mode);
        }
    }

    /// Called on every wake: a finished stop is joined and starts the pending side.
    pub fn poll(&mut self) {
        match std::mem::replace(&mut self.state, State::Off) {
            State::Stopping {
                join,
                done,
                then_start,
            } if done.load(Ordering::Acquire) => {
                // Only the wake call is left on that thread.
                let _ = join.join();
                if then_start {
                    self.start();
                }
            }
            other => self.state = other,
        }
    }

    /// The sink of a side started since the last call, for the feed.
    pub fn take_sink(&mut self) -> Option<OsdSink> {
        self.sink.take()
    }

    /// Stops everything at exit: a running side drops here (its own drop is bounded by
    /// [`STOP_WAIT`]); a stopping one is joined for at most [`STOP_WAIT`]. A pending start is
    /// dropped.
    pub fn shutdown(&mut self) {
        match std::mem::replace(&mut self.state, State::Off) {
            State::Off => {}
            State::Running(side) => drop(side),
            State::Stopping { join, .. } => {
                let _ = join_within(join, STOP_WAIT);
            }
        }
        self.sink = None;
    }

    fn start(&mut self) {
        if let Some(side) = self.starter.start(self.mode) {
            self.sink = Some(S::sink(&side));
            self.state = State::Running(side);
        }
    }

    fn stop(&mut self, then_start: bool) {
        let State::Running(side) = std::mem::replace(&mut self.state, State::Off) else {
            return;
        };
        let done = Arc::new(AtomicBool::new(false));
        let (flag, wake) = (Arc::clone(&done), Arc::clone(&self.wake));
        let spawned = thread::Builder::new()
            .name("plith-osd-stop".to_string())
            .spawn(move || {
                drop(side);
                flag.store(true, Ordering::Release);
                wake();
            });
        match spawned {
            Ok(join) => {
                self.state = State::Stopping {
                    join,
                    done,
                    then_start,
                };
            }
            // The closure, and the side in it, dropped inside the failed spawn: the stop is
            // already done, on this thread.
            Err(_) => {
                if then_start {
                    self.start();
                }
            }
        }
    }
}

impl<S: OsdStart> Drop for OsdSlot<S> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests;
