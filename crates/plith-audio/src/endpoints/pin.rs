//! Which endpoint the service attaches to.

use super::Endpoint;

/// The endpoint id to attach to: the pinned one when it is currently active, otherwise the
/// default. Ids compare case-insensitively (Core Audio ids are GUID strings whose case varies
/// between sources); the returned id is the active endpoint's own spelling. `None` when there
/// is neither a usable pin nor a default.
#[must_use]
pub fn resolve_target(
    pinned: Option<&str>,
    active: &[Endpoint],
    default_id: Option<&str>,
) -> Option<String> {
    let pinned_active = pinned.filter(|id| !id.is_empty()).and_then(|id| {
        active
            .iter()
            .find(|endpoint| endpoint.id.eq_ignore_ascii_case(id))
    });
    match pinned_active {
        Some(endpoint) => Some(endpoint.id.clone()),
        None => default_id.map(str::to_owned),
    }
}
