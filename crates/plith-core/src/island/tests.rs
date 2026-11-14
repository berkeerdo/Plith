use super::*;
use crate::shapes::{PAGES, REST};
use crate::spring::Spring;
use crate::tuning::{BRISK, R1, Tuning};

const H: f64 = 1.0 / 240.0;

/// Steps `secs` seconds at 240 Hz and collects the shape after every frame.
fn run(m: &mut IslandMotion, secs: f64) -> Vec<IslandShape> {
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < secs - 1e-9 {
        m.step(H);
        out.push(m.shape());
        t += H;
    }
    out
}

#[test]
fn per_axis_tuning_follows_each_axis_direction() {
    let mut m = IslandMotion::new(REST, R1);
    m.retarget(PAGES[1]);
    // Step until the width is already past the next page's width while the height lags below it.
    let mut guard = 0;
    while !(m.shape().width > PAGES[2].width + 5.0 && m.shape().height < PAGES[2].height - 2.0) {
        m.step(H);
        guard += 1;
        assert!(guard < 2400, "never reached the mixed-direction moment");
    }
    m.retarget(PAGES[2]);
    let samples = run(&mut m, 4.0);
    assert!(samples.iter().all(|s| s.width >= PAGES[2].width - 1e-9));
    assert!(samples.iter().any(|s| s.height > PAGES[2].height));
}

#[test]
fn retarget_keeps_velocity_on_every_axis() {
    let mut m = IslandMotion::new(REST, R1);
    m.retarget(PAGES[0]);
    run(&mut m, 0.1);
    let before = (
        m.width.spring.velocity(),
        m.height.spring.velocity(),
        m.radius.spring.velocity(),
    );
    assert!(before.0 > 0.0 && before.1 > 0.0 && before.2 > 0.0);
    m.retarget(PAGES[1]);
    let after = (
        m.width.spring.velocity(),
        m.height.spring.velocity(),
        m.radius.spring.velocity(),
    );
    assert_eq!(before.0.to_bits(), after.0.to_bits());
    assert_eq!(before.1.to_bits(), after.1.to_bits());
    assert_eq!(before.2.to_bits(), after.2.to_bits());
}

#[test]
fn shrinking_axis_never_goes_below_target() {
    let mut m = IslandMotion::new(PAGES[0], R1);
    m.retarget(REST);
    let s = run(&mut m, 3.0);
    for x in &s {
        assert!(x.width >= REST.width - 1e-9, "width {}", x.width);
        assert!(x.height >= REST.height - 1e-9, "height {}", x.height);
        assert!(x.radius >= REST.radius - 1e-9, "radius {}", x.radius);
    }
}

#[test]
fn reversal_mid_collapse_is_continuous_and_above_rest() {
    let mut m = IslandMotion::new(PAGES[0], R1);
    m.retarget(REST);
    let mut all = run(&mut m, 0.1);
    m.retarget(PAGES[0]);
    all.extend(run(&mut m, 2.0));
    for pair in all.windows(2) {
        assert!((pair[1].width - pair[0].width).abs() < 15.0);
        assert!((pair[1].height - pair[0].height).abs() < 15.0);
        assert!((pair[1].radius - pair[0].radius).abs() < 15.0);
    }
    for x in &all {
        assert!(x.width >= REST.width - 1e-9);
        assert!(x.height >= REST.height - 1e-9);
        assert!(x.radius >= REST.radius - 1e-9);
    }
}

#[test]
fn expand_settles_between_0_6_and_0_9_s() {
    let mut m = IslandMotion::new(REST, R1);
    m.retarget(PAGES[0]);
    let mut t = 0.0;
    while !m.step(H) {
        t += H;
        assert!(t < 5.0, "never settled");
    }
    assert!((0.6..=0.9).contains(&t), "settled at {t} s");
}

#[test]
fn radius_never_exceeds_half_height() {
    let mut m = IslandMotion::new(PAGES[0], R1);
    m.retarget(REST);
    let mut all = run(&mut m, 2.0);
    m.retarget(PAGES[1]);
    all.extend(run(&mut m, 2.0));
    for x in &all {
        assert!(x.radius <= x.height / 2.0 + 1e-9);
        assert!(x.radius <= x.width / 2.0 + 1e-9);
    }
}

/// Value of a spring that went from `from` to `to` under `tuning`, after `steps` frames.
fn reference(from: f64, to: f64, tuning: Tuning, steps: usize) -> f64 {
    let mut s = Spring::new(from, tuning);
    s.retarget(to, tuning);
    for _ in 0..steps {
        s.step(H);
    }
    s.value()
}

#[test]
fn card_to_card_growth_uses_the_card_tunings() {
    let mut m = IslandMotion::new(PAGES[0], BRISK);
    m.retarget(PAGES[1]);
    for _ in 0..30 {
        m.step(H);
    }
    let want = reference(PAGES[0].height, PAGES[1].height, BRISK.card_grow_h, 30);
    assert!((m.height.spring.value() - want).abs() < 1e-9);
    let plain = reference(PAGES[0].height, PAGES[1].height, BRISK.grow_h, 30);
    assert!((want - plain).abs() > 0.1, "the two tunings must differ");
}

#[test]
fn growth_from_or_to_rest_keeps_the_plain_grow_tunings() {
    let mut m = IslandMotion::new(REST, BRISK);
    m.retarget(PAGES[0]);
    for _ in 0..30 {
        m.step(H);
    }
    let want = reference(REST.height, PAGES[0].height, BRISK.grow_h, 30);
    assert!((m.height.spring.value() - want).abs() < 1e-9);
    // Mid-collapse the current target is REST, so a retarget up is not card to card.
    let mut back = IslandMotion::new(PAGES[0], BRISK);
    back.retarget(REST);
    back.step(H);
    back.retarget(PAGES[1]);
    back.step(H);
    let mut s = Spring::new(PAGES[0].height, BRISK.shrink_h);
    s.retarget(REST.height, BRISK.shrink_h);
    s.step(H);
    s.retarget(PAGES[1].height, BRISK.grow_h);
    s.step(H);
    assert!((back.height.spring.value() - s.value()).abs() < 1e-9);
}
