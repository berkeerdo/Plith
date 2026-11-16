//! The single-owner lock of a journal file: a session-local named mutex per journal path.
//!
//! The mutex is only created, never waited on: its existence is the lock. It lives as long as
//! a handle to it is open, so a killed owner releases it with its handles, and the next start
//! can take the journal over and restore what the dead run left hidden.

use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;

use crate::flyout::journal;

/// Holds the journal while alive; dropping it releases the journal to the next owner.
#[derive(Debug)]
pub(super) struct JournalLock(HANDLE);

// SAFETY: a mutex handle is a process-wide kernel token; it is only closed, from any thread.
unsafe impl Send for JournalLock {}

impl Drop for JournalLock {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Takes the journal at `path` for this suppressor.
///
/// # Errors
/// Another live suppressor (a second notch, or `verify_system_writes --flyout` next to the
/// app) already owns it, or the mutex could not be created. The journal is then left alone.
pub(super) fn acquire(path: &Path) -> Result<JournalLock, String> {
    let name = journal::lock_name(path);
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the name is NUL-terminated and outlives the call; default security.
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) }
        .map_err(|e| format!("journal lock {name}: {e}"))?;
    // SAFETY: read right after the call, before anything can overwrite it.
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let lock = JournalLock(handle);
    if existed {
        return Err(format!(
            "another Plith process owns the flyout journal {} ({name}); refusing to start a \
             second suppressor, which would restore and clear the other's hidden windows",
            path.display()
        ));
    }
    Ok(lock)
}
