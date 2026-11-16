use super::*;

const SHOW: MotionKind = MotionKind::OsdShow;
const HIDE: MotionKind = MotionKind::OsdHide;

#[test]
fn a_fade_opens_records_and_closes_one_motion() {
    let mut track = MotionTrack::default();
    let first = track.frame(SHOW, true);
    assert_eq!(first.start, Some(SHOW));
    assert!(first.record && !first.end_before && !first.end_after);
    let mid = track.frame(SHOW, true);
    assert_eq!(mid.start, None);
    assert!(mid.record && !mid.end_after);
    let last = track.frame(SHOW, false);
    assert!(
        last.record && last.end_after,
        "the settling frame belongs to the motion"
    );
    assert!(!track.is_open());
}

#[test]
fn repaints_without_a_fade_open_no_motion() {
    let mut track = MotionTrack::default();
    let repaint = track.frame(SHOW, false);
    assert_eq!(
        repaint,
        Decision::default(),
        "mute, meter switch, level-only move"
    );
    assert!(!track.is_open());
}

#[test]
fn a_kind_flip_ends_one_motion_and_starts_the_other() {
    let mut track = MotionTrack::default();
    track.frame(HIDE, true);
    let flip = track.frame(SHOW, true);
    assert!(flip.end_before);
    assert_eq!(flip.start, Some(SHOW));
    assert!(flip.record);
}

#[test]
fn end_reports_whether_a_motion_was_open() {
    let mut track = MotionTrack::default();
    assert!(!track.end());
    track.frame(SHOW, true);
    assert!(track.end());
    assert!(!track.is_open());
}
