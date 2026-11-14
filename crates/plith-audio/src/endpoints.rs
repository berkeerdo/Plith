//! Windows Core Audio render endpoints: the active list, the default, the attached endpoint's
//! volume and mute, and change notifications.
//!
//! The decision of which endpoint to attach to is pure ([`resolve_target`]) and unit tested.
//! The COM side lives on one service thread; this slice is read only (writes come later).

#[cfg(windows)]
mod com;
#[cfg(windows)]
mod notify;
mod pin;
#[cfg(windows)]
mod service;
#[cfg(test)]
mod tests;

pub use pin::resolve_target;
#[cfg(windows)]
pub use service::{EndpointService, Status};

/// One active render endpoint. Both names are kept as Core Audio reports them; shortening is
/// the UI's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Core Audio endpoint id (stable per endpoint).
    pub id: String,
    /// Endpoint name, for example `Speakers (Realtek Audio)`.
    pub friendly_name: String,
    /// Device description, for example `Realtek Audio`; empty when it cannot be read.
    pub device_name: String,
}

/// Volume and mute of the attached endpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct AttachedState {
    /// Id of the endpoint the volume belongs to.
    pub id: String,
    /// Master volume scalar, 0.0 to 1.0.
    pub volume: f32,
    /// Whether the endpoint is muted.
    pub muted: bool,
}

/// Everything known about render endpoints at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EndpointState {
    /// Id of the default render endpoint (Multimedia role), if there is one.
    pub default_id: Option<String>,
    /// Every active render endpoint.
    pub active: Vec<Endpoint>,
    /// The endpoint the service follows, with its volume and mute; `None` when no endpoint
    /// could be attached.
    pub attached: Option<AttachedState>,
}
