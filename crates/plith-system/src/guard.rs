//! Panic containment for the `extern "system"` hook procs.
//!
//! A panic may not unwind out of a hook proc (the ABI does not unwind, so the process aborts),
//! and an abort skips every restore: a hidden shell flyout would stay hidden until Explorer
//! restarts. The procs run their body inside [`contain_text`] and, on a panic, disable
//! themselves and record the text in a `FailureSlot` the owner can read. No global panic
//! hook is installed; the default hook still prints the panic once.

#[cfg(test)]
mod tests;

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};

/// Runs `f`, stopping a panic at this frame. Returns `true` when `f` returned normally.
pub fn contain(f: impl FnOnce()) -> bool {
    contain_text(f).is_none()
}

/// Runs `f`, stopping a panic at this frame. Returns the panic text when `f` panicked.
///
/// `AssertUnwindSafe` is sound for the callers: after a panic each proc disables itself and
/// never reads the state it was touching again (the flyout callback only restores through a
/// poison tolerant lock).
pub fn contain_text(f: impl FnOnce()) -> Option<String> {
    catch_unwind(AssertUnwindSafe(f))
        .err()
        .map(|payload| payload_text(payload.as_ref()))
}

/// The message a panic carried: `panic!` with a literal gives `&str`, with arguments `String`.
fn payload_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "panic with a non-text payload".to_owned()
    }
}

/// A failure text written once by a hook thread and read by its owner on any thread.
#[derive(Debug, Clone, Default)]
pub(crate) struct FailureSlot(Arc<Mutex<Option<String>>>);

impl FailureSlot {
    /// Stores `text` unless a failure is already recorded (the first one is the cause).
    pub(crate) fn record(&self, text: String) {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(text);
        }
    }

    /// The recorded failure, if any.
    pub(crate) fn get(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
