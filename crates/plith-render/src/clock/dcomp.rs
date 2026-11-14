//! The compositor clock: `DCompositionWaitForCompositorClock` (Windows 11, build 22000+).

use super::{ClockError, FrameSource, WAIT_TIMEOUT_MS, seconds_since};
use plith_core::clock_choice::ClockKind;
use std::time::Instant;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::DirectComposition::DCompositionWaitForCompositorClock;

/// Success: `STATUS_SUCCESS`, which with no handles is also `WAIT_OBJECT_0 + 0` (clock tick).
const TICKED: u32 = 0;
/// `STATUS_TIMEOUT`, equal to `WAIT_TIMEOUT`.
const TIMED_OUT: u32 = 0x102;

/// Waits on the DWM compositor clock.
#[derive(Debug)]
pub struct DcompClock {
    origin: Instant,
}

impl DcompClock {
    /// A compositor clock reporting times since `origin`.
    #[must_use]
    pub fn new(origin: Instant) -> Self {
        Self { origin }
    }
}

impl FrameSource for DcompClock {
    fn wait(&mut self) -> Result<f64, ClockError> {
        // SAFETY: no handle array is passed (count 0, null pointer), so the call reads no
        // caller memory; the timeout is finite.
        let status =
            unsafe { DCompositionWaitForCompositorClock(None::<&[HANDLE]>, WAIT_TIMEOUT_MS) };
        let raw = seconds_since(self.origin);
        match status {
            TICKED => Ok(raw),
            TIMED_OUT => Err(ClockError::Timeout),
            other => Err(ClockError::Status(other)),
        }
    }

    fn kind(&self) -> ClockKind {
        ClockKind::Dcomp
    }
}
