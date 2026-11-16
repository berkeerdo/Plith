//! Latest-wins hand-off between one producer and one consumer thread.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// A one-value mailbox: `put` replaces whatever is waiting, `take_wait` blocks for it.
pub struct Slot<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

struct State<T> {
    value: Option<T>,
    closed: bool,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Slot<T> {
    /// Creates an empty, open slot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                value: None,
                closed: false,
            }),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stores `value`, replacing any value not yet taken.
    pub fn put(&self, value: T) {
        self.lock().value = Some(value);
        self.ready.notify_all();
    }

    /// Waits up to `timeout` for a value. After `close`, a pending value is still
    /// returned once, then `None` immediately.
    pub fn take_wait(&self, timeout: Duration) -> Option<T> {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        loop {
            if let Some(value) = state.value.take() {
                return Some(value);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if state.closed || left.is_zero() {
                return None;
            }
            state = self
                .ready
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Closes the slot and wakes any waiter.
    pub fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
    }

    /// Whether `close` was called.
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }
}
