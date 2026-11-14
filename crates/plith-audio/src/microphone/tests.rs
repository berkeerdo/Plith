//! Unit tests for the pure microphone layer.

use super::*;
use crate::endpoints::AttachedState;

fn state(muted: bool) -> AttachedState {
    AttachedState {
        id: "mic-1".into(),
        volume: 0.4,
        muted,
    }
}

#[test]
fn no_device_state_is_no_snapshot() {
    assert_eq!(MicSnapshot::from_state("Mic", None), None);
}

#[test]
fn snapshot_carries_name_volume_and_mute() {
    let got = MicSnapshot::from_state("Mic", Some(state(true))).expect("snapshot");
    assert_eq!(got.id, "mic-1");
    assert_eq!(got.name, "Mic");
    assert!(got.muted);
    assert!((got.volume - 0.4).abs() < f32::EPSILON);
}

#[test]
fn unmuted_device_is_distinct_from_no_device() {
    let some = MicSnapshot::from_state("Mic", Some(state(false)));
    assert!(some.is_some_and(|s| !s.muted));
}
