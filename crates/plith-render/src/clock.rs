//! Frame clocks: blocking waits that return at (or near) a vblank, as raw seconds since the
//! render thread's origin. Which source to use is decided in `plith_core::clock_choice`.

mod dcomp;
mod dxgi;
mod timer;

pub use dcomp::DcompClock;
pub use dxgi::DxgiClock;
pub use timer::TimerClock;

use plith_core::clock_choice::ClockKind;
use std::fmt;
use std::time::Instant;

/// Longest single clock wait in milliseconds, so a stop request is seen within this time.
pub const WAIT_TIMEOUT_MS: u32 = 100;

/// Why a clock wait (or opening a clock) failed. Any error demotes the clock.
#[derive(Debug)]
pub enum ClockError {
    /// No tick arrived within [`WAIT_TIMEOUT_MS`].
    Timeout,
    /// The wait returned an unexpected status code (for example display occluded).
    Status(u32),
    /// No DXGI output drives the notch monitor.
    NoOutput,
    /// A Windows call failed.
    Windows(windows::core::Error),
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "no tick within {WAIT_TIMEOUT_MS} ms"),
            Self::Status(code) => write!(f, "wait returned status {code:#x}"),
            Self::NoOutput => write!(f, "no DXGI output for the notch monitor"),
            Self::Windows(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ClockError {}

impl From<windows::core::Error> for ClockError {
    fn from(error: windows::core::Error) -> Self {
        Self::Windows(error)
    }
}

/// A source of frame ticks.
pub trait FrameSource {
    /// Blocks until the next tick and returns the raw wake-up time in seconds since the
    /// render thread's origin.
    ///
    /// # Errors
    /// Returns a [`ClockError`] when no tick arrived; the caller demotes the source.
    fn wait(&mut self) -> Result<f64, ClockError>;

    /// Which kind of source this is.
    fn kind(&self) -> ClockKind;
}

impl<T: FrameSource + ?Sized> FrameSource for Box<T> {
    fn wait(&mut self) -> Result<f64, ClockError> {
        (**self).wait()
    }

    fn kind(&self) -> ClockKind {
        (**self).kind()
    }
}

/// Seconds elapsed since `origin`: the raw time every clock returns.
#[must_use]
pub fn seconds_since(origin: Instant) -> f64 {
    origin.elapsed().as_secs_f64()
}

/// Opens the clock of `kind`. `monitor` is the notch monitor's `HMONITOR` as an integer (used
/// by the DXGI clock) and `hz` the nominal refresh rate (used by the timer).
///
/// # Errors
/// Returns the [`ClockError`] of the failing open; the caller moves to the next kind.
pub fn open_clock(
    kind: ClockKind,
    origin: Instant,
    monitor: isize,
    hz: f64,
) -> Result<Box<dyn FrameSource>, ClockError> {
    Ok(match kind {
        ClockKind::Dcomp => Box::new(DcompClock::new(origin)),
        ClockKind::Dxgi => Box::new(DxgiClock::new(monitor, origin)?),
        ClockKind::Timer => Box::new(TimerClock::new(hz, origin)?),
    })
}
