//! Reads the desktop compositor's frame counters with `DwmGetCompositionTimingInfo`.

use plith_core::dwm::DwmSample;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};

/// The compositor's counters now, or `None` when the call fails (for example no compositor).
#[must_use]
pub fn read() -> Option<DwmSample> {
    let mut info = DWM_TIMING_INFO {
        cbSize: u32::try_from(size_of::<DWM_TIMING_INFO>()).ok()?,
        ..DWM_TIMING_INFO::default()
    };
    // SAFETY: `info` is a live, correctly sized out parameter with `cbSize` set; since Windows
    // 8.1 only the null window handle is accepted.
    unsafe { DwmGetCompositionTimingInfo(HWND::default(), &raw mut info) }.ok()?;
    Some(DwmSample {
        refreshes: info.cRefresh,
        missed: info.cFramesMissed,
        dropped: info.cFramesDropped,
    })
}
