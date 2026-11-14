//! Compositor frame counters, sampled before and after a motion, so a log line can say how many
//! frames the desktop compositor missed or dropped while it ran.

/// A reading of the compositor's cumulative counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DwmSample {
    /// Display refreshes since the compositor started.
    pub refreshes: u64,
    /// Frames the application did not deliver in time.
    pub missed: u64,
    /// Frames the compositor dropped.
    pub dropped: u64,
}

/// What changed between two readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DwmDelta {
    /// Refreshes between the two readings.
    pub refreshes: u64,
    /// Missed frames between the two readings.
    pub missed: u64,
    /// Dropped frames between the two readings.
    pub dropped: u64,
}

impl DwmSample {
    /// The change from `self` to `later`; a counter that went backwards (a reset) gives 0.
    #[must_use]
    pub fn delta(self, later: DwmSample) -> DwmDelta {
        DwmDelta {
            refreshes: later.refreshes.saturating_sub(self.refreshes),
            missed: later.missed.saturating_sub(self.missed),
            dropped: later.dropped.saturating_sub(self.dropped),
        }
    }
}

#[cfg(test)]
mod tests;
