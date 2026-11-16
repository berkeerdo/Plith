//! The deadline for a connected client's `Hello`, enforced from a second thread.
//!
//! The pipe is synchronous, so a silent client keeps the serving thread inside `ReadFile`.
//! [`HelloClock::run`] waits for the armed deadline and then calls `cancel` (in the server:
//! `CancelSynchronousIo` on the serving thread) every few milliseconds while the clock stays
//! armed, because the serving thread may be between two calls. `cancel` runs under the clock's
//! lock and [`HelloClock::disarm`] takes it too, so a cancel never lands after a disarm.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How often an expired, still armed clock cancels again.
const RETRY: Duration = Duration::from_millis(10);

#[derive(Default)]
struct State {
    deadline: Option<Instant>,
    expired: bool,
    quit: bool,
}

/// One deadline at a time, shared by the serving thread and the clock thread.
#[derive(Default)]
pub(crate) struct HelloClock {
    state: Mutex<State>,
    wake: Condvar,
}

impl HelloClock {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Arms the clock to expire at `deadline`.
    pub(crate) fn arm(&self, deadline: Instant) {
        let mut state = self.lock();
        state.deadline = Some(deadline);
        state.expired = false;
        self.wake.notify_all();
    }

    /// Disarms the clock and returns whether it had expired.
    pub(crate) fn disarm(&self) -> bool {
        let mut state = self.lock();
        state.deadline = None;
        std::mem::take(&mut state.expired)
    }

    /// Whether the armed deadline has passed.
    pub(crate) fn expired(&self) -> bool {
        self.lock().expired
    }

    /// Ends [`run`](Self::run).
    pub(crate) fn quit(&self) {
        self.lock().quit = true;
        self.wake.notify_all();
    }

    /// Waits for deadlines until [`quit`](Self::quit); after one passes, marks the clock
    /// expired and calls `cancel` until it is disarmed.
    pub(crate) fn run(&self, cancel: impl Fn()) {
        let mut state = self.lock();
        loop {
            if state.quit {
                return;
            }
            let wait = match state.deadline {
                None => None,
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        state.expired = true;
                        cancel();
                        Some(RETRY)
                    } else {
                        Some(deadline - now)
                    }
                }
            };
            state = match wait {
                None => self
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(time) => {
                    self.wake
                        .wait_timeout(state, time)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }
}

#[cfg(test)]
mod tests;
