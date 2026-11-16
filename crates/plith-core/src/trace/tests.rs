use super::*;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn normalise_maps_minus_sixty_to_zero_db_onto_zero_to_one() {
    assert!(close(normalise(1.0), 1.0));
    assert!(close(normalise(0.001), 0.0));
    assert!(close(normalise(0.031_623), 0.5));
    assert!(close(normalise(0.0001), 0.0));
    assert!(close(normalise(2.0), 1.0));
    assert!(close(normalise(0.0), 0.0));
    assert!(close(normalise(-1.0), 0.0));
    assert!(close(normalise(f32::NAN), 0.0));
}

#[test]
fn push_keeps_thirty_samples_a_second() {
    let mut ring = TraceRing::new();
    for i in 0..240 {
        ring.push(f64::from(i) / 240.0, 0.5);
    }
    let n = ring.samples().len();
    assert!((30..=31).contains(&n), "{n} samples in one second");
}

#[test]
fn a_ring_holds_eight_seconds() {
    let mut ring = TraceRing::new();
    for i in 0..(20 * 60) {
        ring.push(f64::from(i) / 60.0, 0.5);
    }
    assert_eq!(ring.samples().len(), TRACE_LEN);
}

#[test]
fn samples_come_oldest_first() {
    let mut ring = TraceRing::new();
    for i in 0..10 {
        ring.push(f64::from(i) / TRACE_HZ, 0.001 * 10_f32.powi(i / 3));
    }
    let samples = ring.samples();
    assert!(samples.windows(2).all(|w| w[0] <= w[1]), "{samples:?}");
    assert!(samples[0] < samples[samples.len() - 1]);
}

#[test]
fn a_peak_between_samples_raises_the_next_sample() {
    let mut ring = TraceRing::new();
    ring.push(0.0, 0.0);
    ring.push(0.01, 1.0);
    ring.push(0.034, 0.0);
    assert_eq!(ring.samples(), vec![0.0, 1.0]);
}

#[test]
fn a_gap_longer_than_the_window_clears_the_ring() {
    let mut ring = TraceRing::new();
    for i in 0..30 {
        ring.push(f64::from(i) / TRACE_HZ, 1.0);
    }
    ring.push(30.0, 0.0);
    assert_eq!(ring.samples(), vec![0.0]);
}
