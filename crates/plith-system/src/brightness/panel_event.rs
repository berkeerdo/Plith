//! The per-event decision of the built-in panel listener, kept pure so tests need no WMI.

use std::time::Instant;

use crate::flyout::SuppressionWindow;

/// Handles one `WmiMonitorBrightnessEvent`: opens the flyout suppression window (when the
/// consumer wired one) and only then runs `send`, so the native flyout is already marked
/// for hiding when the consumer reacts to the new level.
pub(super) fn on_panel_event(
    window: Option<&SuppressionWindow>,
    now: Instant,
    send: impl FnOnce(),
) {
    if let Some(window) = window {
        window.open(now);
    }
    send();
}
