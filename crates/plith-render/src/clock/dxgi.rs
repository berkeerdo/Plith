//! The DXGI vblank clock: `IDXGIOutput::WaitForVBlank` on the output that drives the notch
//! monitor.

use super::{ClockError, FrameSource, seconds_since};
use plith_core::clock_choice::ClockKind;
use std::time::Instant;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_NOT_FOUND, IDXGIFactory1, IDXGIOutput,
};

/// Waits for the vertical blank of one output.
#[derive(Debug)]
pub struct DxgiClock {
    output: IDXGIOutput,
    origin: Instant,
}

/// Maps `DXGI_ERROR_NOT_FOUND` (the end of an enumeration) to `None`.
fn until_not_found<T>(result: windows::core::Result<T>) -> Result<Option<T>, ClockError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Finds the output whose `DXGI_OUTPUT_DESC.Monitor` is `monitor`, across every adapter.
fn find_output(monitor: isize) -> Result<IDXGIOutput, ClockError> {
    // SAFETY: plain COM calls on interfaces owned by this function; `GetDesc` writes into a
    // value it returns.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut a = 0;
        while let Some(adapter) = until_not_found(factory.EnumAdapters(a))? {
            let mut o = 0;
            while let Some(output) = until_not_found(adapter.EnumOutputs(o))? {
                if output.GetDesc()?.Monitor.0 as isize == monitor {
                    return Ok(output);
                }
                o += 1;
            }
            a += 1;
        }
    }
    Err(ClockError::NoOutput)
}

impl DxgiClock {
    /// Opens the vblank clock of the output showing `monitor` (an `HMONITOR` as an integer).
    ///
    /// # Errors
    /// Returns [`ClockError::NoOutput`] when no output matches, or a failing DXGI call.
    pub fn new(monitor: isize, origin: Instant) -> Result<Self, ClockError> {
        Ok(Self {
            output: find_output(monitor)?,
            origin,
        })
    }
}

impl FrameSource for DxgiClock {
    fn wait(&mut self) -> Result<f64, ClockError> {
        // SAFETY: `output` is a live output owned by this clock. The wait has no timeout
        // parameter; it returns at the next vblank of that output.
        unsafe { self.output.WaitForVBlank()? };
        Ok(seconds_since(self.origin))
    }

    fn kind(&self) -> ClockKind {
        ClockKind::Dxgi
    }
}
