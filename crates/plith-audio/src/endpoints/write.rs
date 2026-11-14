//! Write paths on the attached endpoint: volume and mute. The pure decisions are separate
//! functions and unit tested; the COM calls only run on the service thread.

use windows::Win32::Media::Audio::{ERole, eConsole, eMultimedia};

#[cfg(windows)]
use super::com::Attachment;

/// The roles a default-device switch writes. Communications is deliberately absent: moving it
/// would move call audio (Discord, Teams), which a plain "set default" never means.
pub const DEFAULT_ROLES: [ERole; 2] = [eConsole, eMultimedia];

/// Clamps a requested master volume to 0.0..=1.0; `None` for NaN, which no clamp can place.
#[must_use]
pub fn sanitize_volume(value: f32) -> Option<f32> {
    (!value.is_nan()).then(|| value.clamp(0.0, 1.0))
}

/// The mute value to write after reading `current`. Callers read first rather than remember a
/// value, because other apps and hardware switches change the state at any time.
#[must_use]
pub fn next_mute(current: bool) -> bool {
    !current
}

#[cfg(windows)]
impl Attachment {
    /// Writes the master volume scalar; `false` for NaN or when the endpoint refuses.
    pub(crate) fn set_volume(&self, value: f32) -> bool {
        let Some(value) = sanitize_volume(value) else {
            return false;
        };
        // SAFETY: plain setter on a live interface; a null event context is documented as allowed.
        unsafe {
            self.volume
                .SetMasterVolumeLevelScalar(value, std::ptr::null())
        }
        .is_ok()
    }

    /// Reads the mute, writes the opposite and returns it; `None` when either call fails.
    pub(crate) fn toggle_mute(&self) -> Option<bool> {
        // SAFETY: plain getter and setter on a live interface; a null event context is allowed.
        unsafe {
            let next = next_mute(self.volume.GetMute().ok()?.as_bool());
            self.volume.SetMute(next, std::ptr::null()).ok()?;
            Some(next)
        }
    }
}
