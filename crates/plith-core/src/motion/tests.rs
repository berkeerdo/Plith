use super::*;
use crate::dwm::DwmDelta;
use crate::shapes::{PAGES, REST};

const HZ: f64 = 240.0;
const CALM: FramePhases = FramePhases {
    wait_ms: 0.0,
    late_ms: 0.0,
    draw_ms: 0.3,
    present_ms: 0.1,
};

fn period() -> f64 {
    1.0 / HZ
}

fn stats_with_gaps(gaps_in_periods: &[f64]) -> MotionReport {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    let mut t = 0.0;
    s.record_frame(t, CALM);
    for g in gaps_in_periods {
        t += g * period();
        s.record_frame(t, CALM);
    }
    s.finish()
}

#[test]
fn report_line_matches_the_pinned_format() {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    for i in 0..240 {
        s.record_frame(f64::from(i) / HZ, phases(0.0, 0.0, 0.31, 0.10));
    }
    s.note_visible(100.0 / HZ);
    let line = s.finish().to_string();
    assert_eq!(
        line,
        "motion expand: 240 frames, visible 421 ms, full 1000 ms (240.0 fps at 240 Hz), 0 hitches, \
         worst gap 4.2 ms; draw p50 0.31 p99 0.31 ms; present p50 0.10 p99 0.10 ms; \
         clock dcomp; retargets 0; first wait 0.0 ms; dwm n/a"
    );
}

#[test]
fn first_wait_is_noted_once_with_one_decimal() {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    s.note_first_wait(12.34);
    s.note_first_wait(99.0);
    s.record_frame(0.0, CALM);
    let report = s.finish();
    assert!((report.first_wait_ms - 12.34).abs() < 1e-9);
    assert!(
        report
            .to_string()
            .contains("retargets 0; first wait 12.3 ms; dwm"),
        "{report}"
    );
}

#[test]
fn retargets_and_hitches_keep_their_plural_words() {
    let mut s = MotionStats::new(MotionKind::Page, HZ, "dxgi");
    s.record_frame(0.0, phases(0.0, 0.0, 0.5, 0.2));
    s.record_frame(period() * 3.0, phases(0.0, 0.0, 0.5, 0.2));
    s.note_retarget();
    let line = s.finish().to_string();
    assert!(line.starts_with("motion page: 2 frames"), "{line}");
    assert!(line.contains(", 1 hitches, "), "{line}");
    assert!(
        line.contains("clock dxgi; retargets 1; first wait"),
        "{line}"
    );
}

#[test]
fn a_gap_over_1_5_periods_is_a_hitch() {
    assert_eq!(stats_with_gaps(&[1.0, 1.4, 1.0]).hitches, 0);
    assert_eq!(stats_with_gaps(&[1.0, 1.6, 1.0]).hitches, 1);
    assert_eq!(stats_with_gaps(&[2.0, 3.0]).hitches, 2);
}

#[test]
fn worst_gap_is_the_largest_step_and_zero_for_one_frame() {
    let r = stats_with_gaps(&[1.0, 3.0, 2.0]);
    assert!((r.worst_gap_ms - 3.0 * period() * 1000.0).abs() < 1e-6);
    assert!(stats_with_gaps(&[]).worst_gap_ms.abs() < f64::EPSILON);
}

#[test]
fn duration_and_fps_come_from_the_anim_span_plus_one_period() {
    let r = stats_with_gaps(&[1.0, 1.0, 1.0]);
    assert_eq!(r.frames, 4);
    assert!((r.duration_ms - 4.0 * period() * 1000.0).abs() < 1e-6);
    assert!((r.fps - 240.0).abs() < 1e-6);
    assert!((r.hz - 240.0).abs() < 1e-9);
}

#[test]
fn percentiles_are_nearest_rank() {
    let v: Vec<f64> = (1..=10).map(f64::from).collect();
    assert!((percentile(&v, 50.0) - 5.0).abs() < 1e-12);
    assert!((percentile(&v, 99.0) - 10.0).abs() < 1e-12);
    assert!((percentile(&v, 10.0) - 1.0).abs() < 1e-12);
    assert!((percentile(&v, 0.0) - 1.0).abs() < 1e-12);
    assert!(percentile(&[], 50.0).abs() < f64::EPSILON);
}

#[test]
fn classify_covers_the_four_kinds() {
    let page = PAGES[0];
    let other = PAGES[1];
    assert_eq!(
        MotionKind::classify(page, REST, false),
        MotionKind::Collapse
    );
    assert_eq!(MotionKind::classify(REST, page, false), MotionKind::Expand);
    assert_eq!(MotionKind::classify(page, other, true), MotionKind::Page);
    assert_eq!(MotionKind::classify(page, other, false), MotionKind::Resize);
}

#[test]
fn kind_names_are_the_log_words() {
    assert_eq!(MotionKind::Expand.as_str(), "expand");
    assert_eq!(MotionKind::Collapse.as_str(), "collapse");
    assert_eq!(MotionKind::Page.as_str(), "page");
    assert_eq!(MotionKind::Resize.as_str(), "resize");
}

fn phases(wait: f64, late: f64, draw: f64, present: f64) -> FramePhases {
    FramePhases {
        wait_ms: wait,
        late_ms: late,
        draw_ms: draw,
        present_ms: present,
    }
}

#[test]
fn hitch_detail_keeps_phases_before_and_at_the_gap() {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    let calm = phases(3.9, 0.1, 0.2, 3.1);
    let spike = phases(0.4, 4.2, 0.2, 0.3);
    s.record_frame(0.0, calm);
    s.record_frame(period(), calm);
    s.record_frame(period() * 3.0, spike);
    s.record_frame(period() * 4.0, calm);
    let report = s.finish();
    assert_eq!(report.hitches, 1);
    assert_eq!(report.hitch_details.len(), 1);
    let detail = &report.hitch_details[0];
    assert_eq!(detail.frame, 2);
    assert!((detail.gap_ms - 2.0 * period() * 1000.0).abs() < 1e-6);
    assert_eq!(detail.before, calm);
    assert_eq!(detail.at, spike);
}

#[test]
fn hitch_details_are_capped_and_count_is_exact() {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    let p = phases(1.0, 0.0, 0.2, 0.2);
    let mut t = 0.0;
    s.record_frame(t, p);
    for _ in 0..10 {
        t += 2.0 * period();
        s.record_frame(t, p);
    }
    let report = s.finish();
    assert_eq!(report.hitches, 10);
    assert_eq!(report.hitch_details.len(), MAX_HITCH_DETAILS);
    assert_eq!(report.detail_lines().len(), MAX_HITCH_DETAILS);
}

#[test]
fn report_line_ends_with_dwm_delta() {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    s.record_frame(0.0, FramePhases::default());
    assert!(s.finish().to_string().ends_with("; dwm n/a"));
    s.set_dwm(DwmDelta {
        refreshes: 190,
        missed: 2,
        dropped: 1,
    });
    let line = s.finish().to_string();
    assert!(line.ends_with("; dwm missed 2 dropped 1 of 190"), "{line}");
    assert!(
        line.contains("first wait 0.0 ms; dwm"),
        "prefix must stay put: {line}"
    );
}

#[test]
fn detail_line_format() {
    let mut s = MotionStats::new(MotionKind::Expand, HZ, "dcomp");
    let calm = phases(3.9, 0.1, 0.2, 3.1);
    let spike = phases(0.4, 4.2, 0.2, 0.3);
    for i in 0..57 {
        s.record_frame(f64::from(i) / HZ, calm);
    }
    s.record_frame(58.0 / HZ, spike);
    let lines = s.finish().detail_lines();
    assert_eq!(
        lines,
        vec![String::from(
            "hitch expand #57: gap 8.3 ms; before wait 3.9 late 0.1 draw 0.2 present 3.1; \
             at wait 0.4 late 4.2 draw 0.2 present 0.3"
        )]
    );
}

#[test]
fn visible_duration_ends_at_the_last_visible_frame() {
    let mut s = MotionStats::new(MotionKind::Collapse, HZ, "dcomp");
    for i in 0..200 {
        s.record_frame(f64::from(i) / HZ, CALM);
    }
    assert!(s.finish().visible_ms.abs() < 1e-9, "no visible frame yet");
    s.note_visible(79.0 / HZ);
    let report = s.finish();
    assert!((report.visible_ms - 80.0 * 1000.0 / HZ).abs() < 1e-6);
    assert!((report.duration_ms - 200.0 * 1000.0 / HZ).abs() < 1e-6);
    assert!(
        (report.fps - 240.0).abs() < 1e-6,
        "fps stays over the full duration"
    );
    assert!(
        report
            .to_string()
            .starts_with("motion collapse: 200 frames, visible 333 ms, full 833 ms (240.0 fps"),
        "{report}"
    );
}
