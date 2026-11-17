//! Single instance per session: a named mutex held for the whole run.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::HSTRING;

/// The mutex name of the notch; `Local\` scopes it to the logon session.
pub const INSTANCE_NAME: &str = "Local\\Plith.Notch";

/// Owns the mutex handle; the name is free again when it drops.
#[derive(Debug)]
pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the handle came from `CreateMutexW` and is closed exactly once, here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Takes the mutex `name`. `Ok(None)` means another holder has it.
///
/// # Errors
/// The text of the Windows error when the mutex cannot be created.
pub fn acquire(name: &str) -> Result<Option<InstanceGuard>, String> {
    let wide = HSTRING::from(name);
    // SAFETY: `wide` is a NUL-terminated wide string that outlives the call; no security
    // attributes are passed and the mutex starts unowned.
    let handle = unsafe { CreateMutexW(None, false, &wide) }.map_err(|error| error.to_string())?;
    // SAFETY: reads the calling thread's last error, set by the call just above.
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let guard = InstanceGuard(handle);
    // An existing mutex still gives us a handle; dropping the guard closes it.
    Ok((!exists).then_some(guard))
}

#[cfg(test)]
mod tests;
