use super::*;

const NOMINAL: f64 = 240.0;

fn period() -> f64 {
    1.0 / NOMINAL
}

fn bits_eq(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// Deterministic uniform jitter in `0..3 ms` from a small LCG.
struct Jitter(u32);

impl Jitter {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(self.0 >> 8) / f64::from(1_u32 << 24) * 0.003
    }
}

#[test]
fn jittered_wakeups_snap_to_the_period_grid() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let mut jit = Jitter(7);
    let mut prev: Option<f64> = None;
    for k in 0..1000_u32 {
        let raw = 1.0 + f64::from(k) * period() + jit.next();
        let tick = tl
            .lock(raw)
            .unwrap_or_else(|| panic!("wakeup {k} is a new vblank"));
        assert_eq!(tick.periods, 1, "k = {k}");
        // The grid needs a warm-up to settle on the earliest wakeup. Afterwards a snap to a new
        // earliest wakeup moves a delta by by a small fraction of a period (gain 0.005).
        if let (Some(p), true) = (prev, k >= 100) {
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
        .lock(tick.vblank + period() * 2.74)
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
    assert_eq!(tick.periods, 40);
    assert!(bits_eq(tick.vblank, raw));
    assert!(bits_eq(tick.period, period()));
}

#[test]
fn a_200_ms_stall_reports_its_whole_gap_and_is_a_hitch() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let mut stamper = FrameStamper::default();
    let mut stats =
        crate::motion::MotionStats::new(crate::motion::MotionKind::Expand, NOMINAL, "dcomp");
    let first = tl.lock(1.0).expect("anchor");
    let (t, _) = stamper.stamp(first);
    stats.record_frame(t, 0.3, 0.1);
    let raw = 1.0 + 0.2;
    let tick = tl.lock(raw).expect("resync tick");
    assert_eq!(tick.periods, 48);
    assert!(bits_eq(tick.vblank, raw));
    let (t, dt) = stamper.stamp(tick);
    assert!((dt - 0.2).abs() < 1e-9);
    stats.record_frame(t, 0.3, 0.1);
    let report = stats.finish();
    assert_eq!(report.hitches, 1);
    assert!(
        (report.worst_gap_ms - 200.0).abs() < 1.0,
        "{}",
        report.worst_gap_ms
    );
}

#[test]
fn period_is_always_nominal() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    for k in 0..2000_u32 {
        let raw = 1.0 + f64::from(k) * period() * 1.05;
        if let Some(tick) = tl.lock(raw) {
            assert!(bits_eq(tick.period, period()), "k = {k}");
        }
    }
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
fn off_rhythm_raises_rate_reread() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    assert!(!tl.needs_rate_reread());
    let mut raw = 1.0;
    let mut raised = false;
    for _ in 0..200 {
        // The display really runs at 144 Hz while the nominal rate is 240 Hz.
        raw += 1.0 / 144.0;
        let _ = tl.lock(raw);
        raised |= tl.needs_rate_reread();
    }
    assert!(raised);
    tl.reset();
    assert!(!tl.needs_rate_reread());
}

#[test]
fn jitter_does_not_raise_rate_reread() {
    let mut tl = VsyncTimeline::new(NOMINAL);
    let mut jit = Jitter(99);
    for k in 0..1000_u32 {
        let _ = tl.lock(1.0 + f64::from(k) * period() + jit.next());
        assert!(!tl.needs_rate_reread(), "k = {k}");
    }
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
    let mut last_anim = period();
    for (i, periods) in [1_u32, 1, 2, 1, 3, 1, 1, 2].into_iter().enumerate() {
        k += f64::from(periods);
        // Grid noise: the vblank is off its whole-period position by 0.19 period.
        let noise = if i % 2 == 0 { 0.19 } else { -0.19 };
        let (anim_t, dt) = st.stamp(tick_at(k + noise, periods));
        assert!(
            (dt - f64::from(periods) * period()).abs() < 1e-12,
            "dt = {dt}"
        );
        assert!((anim_t - last_anim - dt).abs() < 1e-12);
        last_anim = anim_t;
    }
}
