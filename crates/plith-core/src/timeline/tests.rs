use super::*;

const NOMINAL: f64 = 240.0;

fn period() -> f64 {
    1.0 / NOMINAL
}

fn bits_eq(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// Deterministic jitter pattern in `0..0.9 ms`: about 0.2 of a period. The grid settles at the
/// mean lateness, and a wakeup more than 0.25 period earlier than that looks spurious, so a
/// wider spread can make a legitimate wakeup return `None`.
fn jitter(k: u32) -> f64 {
    const PATTERN: [f64; 7] = [0.0, 0.0006, 0.0002, 0.0009, 0.0004, 0.0, 0.0007];
    PATTERN[(k % 7) as usize]
}

#[test]
fn jittered_wakeups_snap_to_the_period_grid() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let mut prev: Option<f64> = None;
    for k in 0..500_u32 {
        let raw = 1.0 + f64::from(k) * period() + jitter(k);
        let tick = tl
            .lock(raw)
            .unwrap_or_else(|| panic!("wakeup {k} is a new vblank"));
        assert_eq!(tick.periods, 1, "k = {k}");
        if let Some(p) = prev {
            let steps = (tick.vblank - p) / tick.period;
            assert!(
                (steps - steps.round()).abs() < 0.1,
                "k = {k}, steps = {steps}"
            );
            assert!(steps.round() >= 1.0, "k = {k}");
        }
        prev = Some(tick.vblank);
    }
}

#[test]
fn a_wakeup_up_to_three_quarters_late_belongs_to_its_vblank() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let first = tl.lock(1.0).expect("first lock anchors");
    let raw = first.vblank + period() * 1.74;
    let tick = tl.lock(raw).expect("late wakeup is not spurious");
    assert_eq!(tick.periods, 1);
    let late = tl
        .lock(tick.vblank + period() * 2.76)
        .expect("two periods on");
    assert_eq!(late.periods, 2);
}

#[test]
fn spurious_second_wakeup_returns_none() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let first = tl.lock(1.0).expect("first lock anchors");
    assert!(tl.lock(first.vblank + period() * 0.2).is_none());
}

#[test]
fn first_lock_anchors_at_the_raw_time() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let tick = tl.lock(3.5).expect("first lock anchors");
    assert!(bits_eq(tick.vblank, 3.5));
    assert!(bits_eq(tick.period, period()));
    assert_eq!(tick.periods, 1);
}

#[test]
fn gap_over_30_periods_resyncs() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    tl.lock(1.0).expect("anchor");
    let raw = 1.0 + period() * 40.0;
    let tick = tl.lock(raw).expect("resync tick");
    assert_eq!(tick.periods, 1);
    assert!(bits_eq(tick.vblank, raw));
    assert!(bits_eq(tick.period, period()));
}

#[test]
fn period_stays_within_3_percent_of_nominal() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let mut last = 0.0;
    for k in 0..2000_u32 {
        let raw = 1.0 + f64::from(k) * period() * 1.05;
        if let Some(tick) = tl.lock(raw) {
            assert!(tick.period <= period() * 1.03 + 1e-12, "k = {k}");
            assert!(tick.period >= period() * 0.97 - 1e-12, "k = {k}");
            last = tick.period;
        }
    }
    assert!(
        last > period() * 1.02,
        "period should reach the clamp, got {last}"
    );
}

#[test]
fn set_nominal_rebases_period() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    tl.lock(1.0).expect("anchor");
    tl.set_nominal(60.0);
    let tick = tl.lock(2.0).expect("anchor after set_nominal");
    assert!((tick.period - 1.0 / 60.0).abs() < 1.0 / 60.0 * 0.03);
}

#[test]
fn off_rhythm_wakeups_request_a_rate_reread() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    assert!(!tl.needs_rate_reread());
    let mut grid = tl.lock(1.0).expect("anchor").vblank;
    for _ in 0..40 {
        // Each wakeup lands 0.4 periods away from the grid point it should hit.
        let tick = tl.lock(grid + period() * 1.4).expect("wakeup");
        grid = tick.vblank;
    }
    assert!(tl.needs_rate_reread());
    tl.reset();
    assert!(!tl.needs_rate_reread());
}

fn tick_at(k: f64, periods: u32) -> Tick {
    Tick {
        vblank: k * period(),
        period: period(),
        periods,
    }
}

#[test]
fn first_step_after_rest_is_exactly_one_period() {
    let mut st = FrameStamper::default();
    let (t, dt) = st.stamp(tick_at(10.0, 1));
    assert!(bits_eq(dt, period()));
    assert!(bits_eq(t, 10.0 * period() + period()));
    st.stamp(tick_at(11.0, 1));
    st.reset();
    let (_, dt) = st.stamp(tick_at(500.0, 1));
    assert!(bits_eq(dt, period()));
}

#[test]
fn every_step_is_a_whole_number_of_periods() {
    let mut st = FrameStamper::default();
    st.stamp(tick_at(0.0, 1));
    let mut k = 0.0;
    for periods in [1_u32, 1, 2, 1, 3, 1, 1, 2] {
        k += f64::from(periods);
        let (_, dt) = st.stamp(tick_at(k, periods));
        let steps = dt / period();
        assert!((steps - f64::from(periods)).abs() < 1e-9, "steps = {steps}");
    }
}
