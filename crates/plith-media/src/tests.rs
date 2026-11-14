//! Unit tests for the pure layer: timeline, snapshot assembly and the generation counter.

use std::time::{Duration, SystemTime};

use crate::coalesce::Generation;
use crate::snapshot::{MediaSnapshot, PlaybackFacts, TrackFacts};
use crate::timeline::{
    Timeline, datetime_to_system_time, normalize, position_at, ticks_to_duration,
};

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn at(n: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + secs(n)
}

fn timeline(position: u64, duration: u64, stamp: u64) -> Timeline {
    Timeline {
        position: secs(position),
        duration: secs(duration),
        stamp: at(stamp),
    }
}

#[test]
fn zero_duration_is_none() {
    assert_eq!(
        normalize(secs(0), secs(0), secs(5), Some(at(1)), at(2)),
        None
    );
    assert_eq!(normalize(secs(10), secs(10), secs(5), None, at(2)), None);
    assert_eq!(normalize(secs(10), secs(4), secs(5), None, at(2)), None);
}

#[test]
fn start_time_subtracted() {
    let got = normalize(secs(100), secs(300), secs(130), Some(at(7)), at(9)).expect("timeline");
    assert_eq!(got.duration, secs(200));
    assert_eq!(got.position, secs(30));
}

#[test]
fn missing_stamp_uses_now() {
    let got = normalize(secs(0), secs(60), secs(5), None, at(42)).expect("timeline");
    assert_eq!(got.stamp, at(42));
    let kept = normalize(secs(0), secs(60), secs(5), Some(at(7)), at(42)).expect("timeline");
    assert_eq!(kept.stamp, at(7));
}

#[test]
fn position_before_start_is_zero() {
    let got = normalize(secs(100), secs(300), secs(50), None, at(1)).expect("timeline");
    assert_eq!(got.position, Duration::ZERO);
}

#[test]
fn position_interpolates_only_when_playing() {
    let t = timeline(10, 100, 1000);
    assert_eq!(position_at(&t, true, at(1005)), secs(15));
    assert_eq!(position_at(&t, false, at(1005)), secs(10));
}

#[test]
fn position_clamped_to_duration() {
    let t = timeline(90, 100, 1000);
    assert_eq!(position_at(&t, true, at(5000)), secs(100));
}

#[test]
fn stamp_in_the_future_does_not_go_backwards() {
    let t = timeline(10, 100, 2000);
    assert_eq!(position_at(&t, true, at(1000)), secs(10));
}

#[test]
fn default_datetime_is_no_stamp() {
    assert_eq!(datetime_to_system_time(0), None);
    assert_eq!(datetime_to_system_time(-5), None);
    // 1970-01-01 plus 10 s.
    let ticks = 116_444_736_000_000_000 + 100_000_000;
    assert_eq!(datetime_to_system_time(ticks), Some(at(10)));
}

#[test]
fn negative_ticks_are_zero_duration() {
    assert_eq!(ticks_to_duration(-1), Duration::ZERO);
    assert_eq!(ticks_to_duration(10_000_000), secs(1));
}

fn track() -> TrackFacts {
    TrackFacts {
        title: "Song".into(),
        artist: "Band".into(),
        thumbnail: Some(vec![1, 2, 3]),
    }
}

fn playback() -> PlaybackFacts {
    PlaybackFacts {
        playing: true,
        can_seek: true,
        can_next: true,
        can_prev: false,
    }
}

#[test]
fn assemble_combines_all_reads() {
    let t = timeline(1, 10, 5);
    let got = MediaSnapshot::assemble("app.id", Some(track()), Some(playback()), Some(t));
    assert!(got.has_session);
    assert_eq!(got.source_id, "app.id");
    assert_eq!(got.title, "Song");
    assert_eq!(got.thumbnail, Some(vec![1, 2, 3]));
    assert!(got.playing && got.can_seek && got.can_next && !got.can_prev);
    assert_eq!(got.timeline, Some(t));
}

#[test]
fn assemble_survives_failed_reads() {
    let got = MediaSnapshot::assemble("app.id", None, None, None);
    assert!(got.has_session);
    assert_eq!(got.title, "");
    assert!(!got.playing && !got.can_seek);
    assert_eq!(got.timeline, None);
}

#[test]
fn live_stream_cannot_seek() {
    let got = MediaSnapshot::assemble("app.id", Some(track()), Some(playback()), None);
    assert!(!got.can_seek);
}

#[test]
fn no_session_snapshot_is_empty() {
    let got = MediaSnapshot::none();
    assert!(!got.has_session);
    assert_eq!(got, MediaSnapshot::default());
}

#[test]
fn stale_read_is_detected() {
    let generation = Generation::default();
    let seen = generation.current();
    assert!(generation.is_current(seen));
    generation.bump();
    assert!(!generation.is_current(seen));
    assert!(generation.is_current(generation.current()));
}
