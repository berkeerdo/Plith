//! Newest value wins, one write in flight. Pure: the runner is injected.
//!
//! A DDC/CI write was measured at about 56 ms, so a held key asks for far more values than the
//! hardware can take. A queue would keep changing the screen after the key came up; replacing
//! the pending value ends the screen where the person left it, as fast as the bus allows.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::BrightnessDevice;

/// A unit of work handed to the runner.
pub type Job = Box<dyn FnOnce() + Send>;
/// How the pump gets off the calling thread.
pub type Runner = Box<dyn Fn(Job) + Send + Sync>;
/// Told after every value has been offered to every device.
pub type Observer = Box<dyn Fn(&WriteOutcome) + Send + Sync>;

/// The result of writing one value to all devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    /// The value that was written.
    pub value: u8,
    /// Ids of the devices that refused it.
    pub failed: Vec<String>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct State {
    pending: Option<u8>,
    pumping: bool,
}

struct Inner {
    state: Mutex<State>,
    devices: Mutex<Vec<Box<dyn BrightnessDevice>>>,
    observer: Observer,
}

impl Inner {
    /// Writes the pending value until none is left. Only one pump runs at a time.
    fn pump(&self) {
        loop {
            let value = {
                let mut state = locked(&self.state);
                let Some(value) = state.pending.take() else {
                    state.pumping = false;
                    return;
                };
                value
            };
            let mut failed = Vec::new();
            for device in locked(&self.devices).iter_mut() {
                // A refusal is reported, never retried and never stops the other devices.
                if !device.write(value) {
                    failed.push(device.id().to_owned());
                }
            }
            (self.observer)(&WriteOutcome { value, failed });
        }
    }
}

/// Writes to every device, latest value wins.
///
/// [`request`](Self::request) never blocks on a device: it stores the value and, when no pump
/// is running, hands one to the runner. A request that arrives while a write is in flight only
/// replaces the pending value; the running pump picks it up.
pub struct CoalescingWriter {
    inner: Arc<Inner>,
    runner: Runner,
}

impl CoalescingWriter {
    /// Creates a writer over `devices`; `runner` executes the pump off the caller's thread and
    /// `observer` hears each outcome.
    #[must_use]
    pub fn new(
        devices: Vec<Box<dyn BrightnessDevice>>,
        runner: Runner,
        observer: Observer,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                devices: Mutex::new(devices),
                observer,
            }),
            runner,
        }
    }

    /// Asks for `percent` (clamped to 100).
    pub fn request(&self, percent: u8) {
        {
            let mut state = locked(&self.inner.state);
            state.pending = Some(percent.min(100));
            if state.pumping {
                return;
            }
            state.pumping = true;
        }
        let inner = Arc::clone(&self.inner);
        (self.runner)(Box::new(move || inner.pump()));
    }
}
