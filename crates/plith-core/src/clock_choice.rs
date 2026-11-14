//! Frame clock choice: which sources to try, in order, and when one is too fast to trust.

use crate::launch::ClockRequest;

/// A wait shorter than this fraction of a period is too fast to be a vblank.
const FAST_FRACTION: f64 = 0.15;
/// Consecutive fast waits that demote a clock.
const FAST_LIMIT: u32 = 3;

/// A frame clock source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClockKind {
    /// `DCompositionWaitForCompositorClock`.
    Dcomp,
    /// `IDXGIOutput::WaitForVBlank` on the notch monitor.
    Dxgi,
    /// A high-resolution waitable timer at the nominal period.
    Timer,
}

impl ClockKind {
    /// The word used in the motion log line.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dcomp => "dcomp",
            Self::Dxgi => "dxgi",
            Self::Timer => "timer",
        }
    }
}

/// The sources to try, best first. A remote session gets the timer only (no real vblank);
/// a named request goes first, followed by the others in the automatic order.
#[must_use]
pub fn clock_order(request: ClockRequest, remote: bool) -> Vec<ClockKind> {
    if remote {
        return vec![ClockKind::Timer];
    }
    let auto = [ClockKind::Dcomp, ClockKind::Dxgi, ClockKind::Timer];
    let first = match request {
        ClockRequest::Auto => return auto.to_vec(),
        ClockRequest::Dcomp => ClockKind::Dcomp,
        ClockRequest::Dxgi => ClockKind::Dxgi,
        ClockRequest::Timer => ClockKind::Timer,
    };
    std::iter::once(first)
        .chain(auto.into_iter().filter(|k| *k != first))
        .collect()
}

/// Watches tick intervals for a clock that returns without waiting for a vblank.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClockHealth {
    fast: u32,
}

impl ClockHealth {
    /// Records one tick interval of `waited` seconds (raw wake to the previous raw wake of the
    /// same source) on a display with `period`; true means demote
    /// (the third consecutive interval under 0.15 period). The streak restarts after a demotion.
    pub fn observe(&mut self, waited: f64, period: f64) -> bool {
        if waited < FAST_FRACTION * period {
            self.fast += 1;
            if self.fast >= FAST_LIMIT {
                self.fast = 0;
                return true;
            }
        } else {
            self.fast = 0;
        }
        false
    }
}

#[cfg(test)]
mod tests;
