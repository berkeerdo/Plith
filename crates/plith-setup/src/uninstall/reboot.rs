//! Deleting a path at the next restart, the only way to remove the running setup image.

use std::path::Path;
use windows::Win32::Storage::FileSystem::{MOVEFILE_DELAY_UNTIL_REBOOT, MoveFileExW};
use windows::core::{HSTRING, PCWSTR};

/// What the uninstall needs for the files it cannot delete now. A fake stands in for it in tests.
pub(super) trait RebootDelete {
    /// Asks Windows to delete `path` at the next restart. Files and then their folders must be
    /// scheduled in that order.
    ///
    /// # Errors
    /// The text of the Windows error.
    fn schedule(&mut self, path: &Path) -> Result<(), String>;
}

/// The real call.
pub(super) struct SystemRebootDelete;

impl RebootDelete for SystemRebootDelete {
    fn schedule(&mut self, path: &Path) -> Result<(), String> {
        let wide = HSTRING::from(path.as_os_str());
        // SAFETY: a NUL-terminated wide path alive for the call; a null new name means delete.
        unsafe {
            MoveFileExW(
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                MOVEFILE_DELAY_UNTIL_REBOOT,
            )
        }
        .map_err(|error| format!("cannot schedule {}: {error}", path.display()))
    }
}
