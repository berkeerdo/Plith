//! Windows Core Audio render endpoints: the active list, the default, the attached endpoint's
//! volume and mute, and change notifications.
//!
//! The decision of which endpoint to attach to is pure ([`resolve_target`]) and unit tested.
//! The COM side lives on one service thread, which also performs the writes (volume, mute,
//! default endpoint) so every COM call stays on the thread that owns the attachment.

#[cfg(windows)]
pub(crate) mod com;
#[cfg(windows)]
pub(crate) mod notify;
mod pin;
#[cfg(windows)]
mod policy;
#[cfg(windows)]
mod service;
#[cfg(test)]
mod tests;
mod write;

pub use pin::resolve_target;
#[cfg(windows)]
pub use service::{EndpointService, Status};
#[cfg(windows)]
pub use write::DEFAULT_ROLES;
pub use write::{all_succeeded, next_mute, sanitize_volume};

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
