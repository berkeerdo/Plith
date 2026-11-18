use super::{join_within, level_message};
use plith_core::osd::{OsdLevel, OsdMeter};
use plith_ipc::{Message, Meter, encode};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn a_finished_thread_is_joined() {
    let thread = thread::spawn(|| {});
    assert!(matches!(
        join_within(thread, Duration::from_secs(5)),
        Some(Ok(()))
    ));
}

#[test]
fn a_panicked_thread_is_reported() {
    let thread = thread::spawn(|| panic!("supervisor test panic"));
    assert!(matches!(
        join_within(thread, Duration::from_secs(5)),
        Some(Err(_))
    ));
}

#[test]
fn a_stuck_thread_is_left_behind_after_the_timeout() {
    let (release, wait) = mpsc::channel::<()>();
    let thread = thread::spawn(move || {
        let _ = wait.recv();
    });
    let start = Instant::now();
    assert!(join_within(thread, Duration::from_millis(50)).is_none());
    let waited = start.elapsed();
    assert!(waited >= Duration::from_millis(50));
    assert!(waited < Duration::from_secs(2));
    // Let the detached thread end.
    drop(release);
}

#[test]
fn a_volume_level_maps_field_by_field() {
    let message = level_message(OsdLevel {
        meter: OsdMeter::Volume,
        percent: 42,
        muted: true,
        label: String::from("Speakers"),
        muted_word: String::from("STUMM"),
    });
    assert_eq!(
        message,
        Message::Level {
            meter: Meter::Volume,
            percent: 42,
            muted: true,
            label: String::from("Speakers"),
            muted_word: String::from("STUMM"),
        }
    );
}

#[test]
fn a_brightness_level_maps_to_the_brightness_meter() {
    let message = level_message(OsdLevel {
        meter: OsdMeter::Brightness,
        percent: 100,
        muted: false,
        label: String::from("PARLAKLIK"),
        muted_word: String::new(),
    });
    assert!(matches!(
        message,
        Message::Level {
            meter: Meter::Brightness,
            percent: 100,
            muted: false,
            ..
        }
    ));
}

#[test]
fn a_full_length_label_still_encodes() {
    let label = "a".repeat(plith_core::osd::MAX_LABEL_BYTES);
    let message = level_message(OsdLevel {
        meter: OsdMeter::Volume,
        percent: 0,
        muted: false,
        label,
        muted_word: "é".repeat(plith_core::osd::MAX_LABEL_BYTES / 2),
    });
    assert!(encode(&message).is_ok());
}
