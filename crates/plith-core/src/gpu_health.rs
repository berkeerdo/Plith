//! Tracks present failures and decides when to recreate the device or fall back to WARP.

/// What the render loop should do after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuAction {
    /// Nothing to do.
    None,
    /// Recreate the device and swap chain.
    Recreate,
    /// Recreate on the WARP software adapter.
    FallBackToWarp,
}

/// Counts failures since the last reset.
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuHealth {
    failures: u32,
    good_streak: u32,
}

/// Failures since the last reset that trigger the WARP fallback.
const FALLBACK_FAILURES: u32 = 3;
/// Consecutive good frames that clear the failure count.
const RESET_GOOD_FRAMES: u32 = 600;

impl GpuHealth {
    /// Records the outcome of one frame.
    pub fn record(&mut self, ok: bool) -> GpuAction {
        if ok {
            self.good_streak = self.good_streak.saturating_add(1);
            if self.good_streak >= RESET_GOOD_FRAMES {
                self.failures = 0;
            }
            return GpuAction::None;
        }
        self.good_streak = 0;
        self.failures += 1;
        if self.failures >= FALLBACK_FAILURES {
            self.failures = 0;
            GpuAction::FallBackToWarp
        } else {
            GpuAction::Recreate
        }
    }
}

#[cfg(test)]
mod tests;
