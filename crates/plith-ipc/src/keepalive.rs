//! Keepalive timing between the notch and the helper. Pure.
//!
//! The pipe has one instance, so an admitted client that goes silent would hold it forever.
//! The server drops an admitted client that sends nothing for [`IDLE_TIMEOUT`]; a live
//! [`OsdLink`](crate::OsdLink) sends [`Message::Ping`](crate::Message::Ping) after
//! [`KEEPALIVE`] without another write, so the notch is never the one dropped.

use std::time::{Duration, Instant};

/// How long the client may go without writing before it sends a `Ping`.
pub const KEEPALIVE: Duration = Duration::from_secs(10);

/// How long the server keeps an admitted client that sends nothing at all.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether a `Ping` is due: the last write was at least [`KEEPALIVE`] before `now`.
#[must_use]
pub fn ping_due(last_write: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_write) >= KEEPALIVE
}

#[cfg(test)]
mod tests;
