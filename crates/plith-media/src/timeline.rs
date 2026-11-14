//! The position of the current track, as a stamped reading.
//!
//! SMTC reports where the track was at `stamp`, not where it is now. [`position_at`]
//! interpolates. A timeline only exists with a positive duration, so no consumer has to guard a
//! division.

use std::time::{Duration, SystemTime};

/// Where the current track was, and when that was true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeline {
    /// Position at `stamp`, with the track start already subtracted.
    pub position: Duration,
    /// Length of the track; always greater than zero.
    pub duration: Duration,
    /// When `position` was true.
    pub stamp: SystemTime,
}

/// Builds a timeline from raw SMTC values, or `None` when there is nothing to draw.
///
/// `None` for a zero or negative duration (a live stream, or a source that reports no end): a
/// bar of unknown length is a lie. `start` is subtracted because it is not always zero for
/// chaptered content. A missing `last_updated` becomes `now`: left at the default epoch the
/// interpolation would pin every bar to the end of its track.
#[must_use]
pub fn normalize(
    start: Duration,
    end: Duration,
    position: Duration,
    last_updated: Option<SystemTime>,
    now: SystemTime,
) -> Option<Timeline> {
    let duration = end.saturating_sub(start);
    if duration.is_zero() {
        return None;
    }
    Some(Timeline {
        position: position.saturating_sub(start).min(duration),
        duration,
        stamp: last_updated.unwrap_or(now),
    })
}

/// The interpolated position at `now`: the stamped position plus the time since the stamp while
/// `playing`, never past the end.
#[must_use]
pub fn position_at(timeline: &Timeline, playing: bool, now: SystemTime) -> Duration {
    if !playing {
        return timeline.position;
    }
    let elapsed = now.duration_since(timeline.stamp).unwrap_or(Duration::ZERO);
    (timeline.position + elapsed).min(timeline.duration)
}

/// Converts `WinRT` `TimeSpan` ticks (100 ns) to a duration; negative becomes zero.
#[must_use]
pub fn ticks_to_duration(ticks: i64) -> Duration {
    u64::try_from(ticks).map_or(Duration::ZERO, |t| {
        Duration::from_nanos(t.saturating_mul(100))
    })
}

/// Converts `WinRT` `DateTime` ticks (100 ns since 1601-01-01 UTC) to a time; `None` for the
/// default value 0 and anything before the Unix epoch, which sources use for "never updated".
#[must_use]
pub fn datetime_to_system_time(ticks: i64) -> Option<SystemTime> {
    const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
    let since_unix = u64::try_from(ticks.checked_sub(UNIX_EPOCH_TICKS)?).ok()?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(since_unix.saturating_mul(100)))
}
