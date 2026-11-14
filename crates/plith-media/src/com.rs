//! The worker thread's COM apartment.

use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

/// Owns COM for the worker thread; dropped last.
pub(crate) struct ComGuard;

impl ComGuard {
    pub(crate) fn init() -> windows::core::Result<Self> {
        // SAFETY: plain apartment initialization on the calling thread, balanced by `Drop`.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: balances the successful `CoInitializeEx` in `init`.
        unsafe { CoUninitialize() };
    }
}
