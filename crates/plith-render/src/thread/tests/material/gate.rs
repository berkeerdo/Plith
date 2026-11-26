//! The content gate in the loop: a Halka open shows its page only once the ripples are gone
//! (no frame has both), Gece's content handover is the one it always had, frame by frame.

use super::super::expanded;
use super::super::harness::*;
use super::{brisk, brisk_with, calls_to_open, halka_at_rest, in_halka};
use crate::paint::PaintFrame;
use plith_core::scene::Scene;
use plith_core::spring::Spring;
use plith_core::tuning::BRISK;

/// The most the Halka content may lag Gece's in a brisk expand, at every alpha level, in ms.
/// The floor is the press (progress 0.85 at about 152 ms), the hold and the fade: 312.5 ms
/// measured (170 before the slower ripples of 2026-11-27).
const MAX_LAG_MS: f64 = 320.0;

fn ripples_show(frame: &PaintFrame) -> bool {
    frame.material.ripples.iter().any(|r| *r > 0.0)
}

/// Asserts that no frame shows a ripple and page content together.
fn assert_apart(frames: &[PaintFrame]) {
    for (i, f) in frames.iter().enumerate() {
        assert!(
            !(ripples_show(f) && f.content_opacity > 0.0),
            "frame {i}: ripples {:?} with content {}",
            f.material.ripples,
            f.content_opacity
        );
    }
}

/// The expand from rest to page 0 of `run`: (ms since the expand, frame) per motion frame.
fn expand_frames(run: &Run) -> Vec<(f64, PaintFrame)> {
    let before = run.frames().len();
    let start = run.harness.total();
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    let (frames, calls) = (run.frames(), run.calls_at());
    (before..frames.len())
        .map(|i| {
            let waits = f64::from(u32::try_from(calls[i] - start).expect("few calls"));
            (waits * PERIOD * 1000.0, frames[i].clone())
        })
        .collect()
}

/// The first time the content reaches `level`.
fn first_at(frames: &[(f64, PaintFrame)], level: f32) -> f64 {
    frames
        .iter()
        .find(|(_, f)| f.content_opacity >= level)
        .map(|(t, _)| *t)
        .expect("the content reaches the level")
}

#[test]
fn a_halka_open_never_shows_ripples_and_page_together() {
    let run = halka_at_rest();
    let frames = expand_frames(&run);
    let painted: Vec<PaintFrame> = frames.iter().map(|(_, f)| f.clone()).collect();
    assert!(painted.iter().any(ripples_show), "the ripples ran");
    assert_apart(&painted);
    let last_ripple = frames.iter().rev().find(|(_, f)| ripples_show(f));
    let first_page = frames.iter().find(|(_, f)| f.content_opacity > 0.0);
    let (Some((gone, _)), Some((shown, _))) = (last_ripple, first_page) else {
        panic!("ripples and page both expected");
    };
    assert!(shown > gone, "page at {shown} ms, last ripple at {gone} ms");
    let end = painted.last().expect("frames");
    assert!(
        (end.content_opacity - 1.0).abs() < f32::EPSILON,
        "page fully in"
    );
    let _ = run.stop();
}

#[test]
fn the_halka_content_lags_gece_by_the_ripples_only() {
    let gece = brisk();
    gece.wait_frames(1);
    let plain = expand_frames(&gece);
    let _ = gece.stop();
    let halka = halka_at_rest();
    let material = expand_frames(&halka);
    let _ = halka.stop();
    for level in [0.01, 0.5, 0.9, 0.99] {
        let (was, now) = (first_at(&plain, level), first_at(&material, level));
        eprintln!(
            "content {level}: gece {was:.1} ms, halka {now:.1} ms, lag {:.1} ms",
            now - was
        );
        assert!(now - was <= MAX_LAG_MS, "lag at {level}: {was} then {now}");
    }
}

#[test]
fn gece_content_keeps_its_handover_frame_by_frame() {
    let run = brisk();
    run.wait_frames(1);
    let frames = expand_frames(&run);
    let mut spring = Spring::new(0.0, BRISK.opacity).with_eps(0.002, 0.01);
    spring.retarget(1.0, BRISK.opacity);
    for (i, (_, f)) in frames.iter().enumerate() {
        spring.step(PERIOD);
        // Allowed: an opacity in 0..=1.
        #[allow(clippy::cast_possible_truncation)]
        let want = spring.value().clamp(0.0, 1.0) as f32;
        assert!(
            (f.content_opacity - want).abs() < 1e-6,
            "frame {i}: {} vs {want}",
            f.content_opacity
        );
    }
    let _ = run.stop();
}

#[test]
fn interrupts_keep_ripples_and_page_apart() {
    // A collapse 20 waits into the open.
    let run = in_halka(brisk_with(vec![(20, Scene::default())], None));
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    assert_apart(&run.frames());
    let _ = run.stop();
    // A reopen late in a collapse (the reveal has shrunk below 10 %): the ripples replay.
    let reopen = calls_to_open() + 150;
    let run = in_halka(brisk_with(vec![(reopen, expanded(0))], None));
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    let before = run.frames().len();
    run.mailbox.publish(Scene::default());
    let _ = run.report();
    let frames = run.frames();
    assert!(
        frames[before..].iter().any(ripples_show),
        "the reopen replayed"
    );
    assert_apart(&frames);
    let end = frames.last().expect("frames");
    assert!(
        (end.content_opacity - 1.0).abs() < f32::EPSILON,
        "page in again"
    );
    let _ = run.stop();
}
