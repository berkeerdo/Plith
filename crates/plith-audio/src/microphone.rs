//! The default capture device (microphone): its name, mute and volume, change notifications
//! and a mute toggle.
//!
//! The Windows side follows the Communications capture role, as the WPF app did. No capture
//! device is a real state on a desktop, so the snapshot is `None` then and must never be shown
//! as "unmuted".

#[cfg(windows)]
mod service;
#[cfg(test)]
mod tests;

#[cfg(windows)]
pub use crate::endpoints::Status;
#[cfg(windows)]
pub use service::MicrophoneService;

use crate::endpoints::AttachedState;

/// One reading of the default capture device.
#[derive(Debug, Clone, PartialEq)]
pub struct MicSnapshot {
    /// Core Audio endpoint id.
    pub id: String,
    /// Endpoint name as Core Audio reports it; shortening is the UI's decision.
    pub name: String,
    /// Whether the endpoint itself is muted (an app muting its own stream does not show here).
    pub muted: bool,
    /// Master volume scalar, 0.0 to 1.0.
    pub volume: f32,
}

impl MicSnapshot {
    /// Builds a snapshot from an endpoint name and its attached state; `None` when there is no
    /// state, which means there is no capture device.
    #[must_use]
    pub fn from_state(name: &str, state: Option<AttachedState>) -> Option<Self> {
        let state = state?;
        Some(Self {
            id: state.id,
            name: name.to_owned(),
            muted: state.muted,
            volume: state.volume,
        })
    }
}
