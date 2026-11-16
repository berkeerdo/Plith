//! Small helpers shared by the Windows pipe code.

use windows::Win32::Foundation::{CloseHandle, HANDLE};

/// Encodes `text` as a NUL-terminated UTF-16 string.
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// An owned kernel handle, closed on drop.
pub(crate) struct OwnedHandle(pub(crate) HANDLE);

// SAFETY: a kernel handle is a process-wide token that any thread may use or close.
unsafe impl Send for OwnedHandle {}
// SAFETY: see `Send`; the wrapper adds no unsynchronised state.
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
