use super::*;

#[test]
fn p99_takes_the_nearest_rank() {
    assert!(p99(&[]).abs() < f64::EPSILON);
    let samples: Vec<f64> = (1..=100).map(f64::from).collect();
    assert!((p99(&samples) - 99.0).abs() < f64::EPSILON);
    assert!((p99(&[3.0, 1.0, 2.0]) - 3.0).abs() < f64::EPSILON);
}

#[test]
fn median_takes_the_middle_sample() {
    assert!(median(&[]).abs() < f64::EPSILON);
    assert!((median(&[9.0, 1.0, 4.0]) - 4.0).abs() < f64::EPSILON);
}

#[test]
fn summary_reports_frames_fps_hitches_worst_gap_and_present_p99() {
    let mut second = Second::default();
    for _ in 0..238 {
        second.push(4.17, 0.2);
    }
    second.push(6.0, 0.2);
    second.push(9.5, 1.25);
    assert_eq!(
        second.summary("plain", 1.0),
        "motion spike plain: 240 frames (240.0 fps), 1 hitches, worst gap 9.50 ms, \
         present p99 0.20 ms"
    );
}

#[test]
fn a_gap_at_one_and_a_half_periods_is_not_a_hitch() {
    let mut second = Second::default();
    for _ in 0..3 {
        second.push(4.0, 0.1);
    }
    second.push(6.0, 0.1);
    assert_eq!(second.hitches(), 0);
    second.push(6.1, 0.1);
    assert_eq!(second.hitches(), 1);
}

#[test]
fn the_first_second_starts_at_the_first_tick() {
    let mut meter = Meter::default();
    assert_eq!(meter.tick(5.0, 0.1, "plain"), None);
    assert_eq!(meter.tick(5.5, 0.1, "plain"), None);
    assert_eq!(
        meter.tick(6.0, 0.1, "plain").as_deref(),
        Some(
            "motion spike plain: 2 frames (2.0 fps), 0 hitches, worst gap 500.00 ms, \
             present p99 0.10 ms"
        )
    );
}

#[test]
fn flush_reports_the_partial_second_once() {
    let mut meter = Meter::default();
    meter.tick(1.0, 0.1, "inband");
    meter.tick(1.25, 0.3, "inband");
    assert_eq!(
        meter.flush(1.5, "inband").as_deref(),
        Some(
            "motion spike inband: 1 frames (2.0 fps), 0 hitches, worst gap 250.00 ms, \
             present p99 0.30 ms"
        )
    );
    assert_eq!(meter.flush(1.6, "inband"), None);
}

#[test]
fn flush_without_frames_is_silent() {
    let mut meter = Meter::default();
    assert_eq!(meter.flush(2.0, "plain"), None);
    meter.tick(2.0, 0.1, "plain");
    assert_eq!(meter.flush(2.1, "plain"), None);
}
