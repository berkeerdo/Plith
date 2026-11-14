use super::*;

#[test]
fn p99_takes_the_nearest_rank() {
    assert!(p99(&[]).abs() < f64::EPSILON);
    let samples: Vec<f64> = (1..=100).map(f64::from).collect();
    assert!((p99(&samples) - 99.0).abs() < f64::EPSILON);
    assert!((p99(&[3.0, 1.0, 2.0]) - 3.0).abs() < f64::EPSILON);
}

#[test]
fn summary_reports_frames_fps_worst_gap_and_present_p99() {
    let mut second = Second::default();
    for _ in 0..239 {
        second.push(4.17, 0.2);
    }
    second.push(9.5, 1.25);
    assert_eq!(
        second.summary("plain", 1.0),
        "motion spike plain: 240 frames (240.0 fps), worst gap 9.50 ms, present p99 0.20 ms"
    );
}
