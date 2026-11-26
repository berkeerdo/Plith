//! The material timeline inside the loop: a Halka expand runs until its ripples end, a plain
//! theme settles as it did before materials existed, and the motion line names the theme.

use super::expanded;
use super::harness::*;
use super::palette::handle_of;
use crate::paint::PaintFrame;
use plith_core::draw::{ArtId, ArtPixels};
use plith_core::material::art::{MaterialArt, ThemePaint};
use plith_core::material::{FADE_MS, HOLD_MS, MaterialLayers, PRESS_MS, RIPPLE_START_MS};
use plith_core::motion::MotionKind;
use plith_core::scene::{Scene, Suppression};
use plith_core::theme::Palette;
use plith_core::theme::registry::ThemeId;
use plith_core::tuning::BRISK;
use std::sync::Arc;

/// Frames of the fixed demo expand (rest to page 0, BRISK, 240 Hz) and of its collapse,
/// measured on the loop before the material timeline existed (tree at 497d9da).
const GECE_EXPAND_FRAMES: usize = 144;
const GECE_COLLAPSE_FRAMES: usize = 168;

/// Time after `expand` when the last ripple has faded out.
const RIPPLES_DONE_MS: f64 = RIPPLE_START_MS[2] + PRESS_MS + HOLD_MS + FADE_MS;

fn brisk() -> Run {
    start(Setup {
        tune: Some(BRISK),
        ..Setup::default()
    })
}

fn pixels(id: u64) -> Arc<ArtPixels> {
    Arc::new(ArtPixels {
        id: ArtId(id),
        width: 1,
        height: 1,
        bgra: Arc::from([0u8, 0, 0, 255]),
    })
}

/// A Halka paint with stand-in pictures: the fake painter never draws them.
fn halka() -> ThemePaint {
    let art = MaterialArt {
        fabric: pixels(1),
        rings: [pixels(2), pixels(3), pixels(4)],
        disc: pixels(5),
        pill: pixels(6),
    };
    let id = ThemeId::parse("halka").expect("id");
    ThemePaint::new(
        id,
        Palette::default(),
        Palette::default(),
        Some(Arc::new(art)),
    )
}

fn assert_layers(got: MaterialLayers, want: MaterialLayers, what: &str) {
    let close = (got.reveal - want.reveal).abs() < 1e-3
        && got
            .ripples
            .iter()
            .zip(want.ripples)
            .all(|(a, b)| (a - b).abs() < 1e-3)
        && (got.disc - want.disc).abs() < 1e-3
        && (got.disc_lift - want.disc_lift).abs() < 1e-2
        && (got.pill - want.pill).abs() < 1e-3;
    assert!(close, "{what}: got {got:?}, want {want:?}");
}

fn last(frames: &[PaintFrame]) -> MaterialLayers {
    frames.last().expect("a painted frame").material
}

/// A Halka run whose collapsed frame was repainted in Halka.
fn halka_at_rest() -> Run {
    let run = brisk();
    run.wait_frames(1);
    handle_of(&run).set_theme(halka());
    run.wait_frames(2);
    run.assert_parked();
    run
}

#[test]
fn gece_settles_as_before() {
    let run = brisk();
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let open = run.report();
    assert_eq!(open.kind, MotionKind::Expand);
    assert_eq!(open.frames, GECE_EXPAND_FRAMES, "{open}");
    assert!(open.to_string().ends_with("; theme gece"), "{open}");
    run.mailbox.publish(Scene::default());
    let back = run.report();
    assert_eq!(back.frames, GECE_COLLAPSE_FRAMES, "{back}");
    for frame in run.frames() {
        assert_eq!(
            frame.material,
            MaterialLayers::OPEN,
            "a plain theme paints no motion"
        );
    }
    let _ = run.stop();
}

#[test]
fn halka_at_rest_collapsed_paints_the_pill_only() {
    let run = halka_at_rest();
    assert_layers(
        last(&run.frames()),
        MaterialLayers::COLLAPSED,
        "collapsed at rest",
    );
    let _ = run.stop();
}

#[test]
fn halka_expand_is_not_settled_until_ripples_end() {
    let run = halka_at_rest();
    let before = run.frames().len();
    run.mailbox.publish(expanded(0));
    let open = run.report();
    assert_eq!(open.kind, MotionKind::Expand);
    assert!(open.frames > GECE_EXPAND_FRAMES, "{open}");
    assert!(
        open.duration_ms >= RIPPLES_DONE_MS,
        "the ripples end at {RIPPLES_DONE_MS} ms: {open}"
    );
    assert!(open.to_string().ends_with("; theme halka"), "{open}");
    let frames = run.frames();
    let motion = &frames[before..];
    assert!(
        motion
            .iter()
            .any(|f| f.material.ripples.iter().all(|r| *r > 0.99)),
        "all three ripples pressed at once"
    );
    assert!(
        motion
            .iter()
            .any(|f| f.material.pill < 1.0 && f.material.disc > 0.5),
        "the disc shows as the pill fades"
    );
    assert_layers(last(&frames), MaterialLayers::OPEN, "open at rest");
    let _ = run.stop();
}

#[test]
fn halka_collapse_ends_on_the_pill() {
    let run = halka_at_rest();
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.mailbox.publish(Scene::default());
    let back = run.report();
    assert_eq!(back.kind, MotionKind::Collapse);
    assert!(back.to_string().ends_with("; theme halka"), "{back}");
    assert_layers(
        last(&run.frames()),
        MaterialLayers::COLLAPSED,
        "collapsed again",
    );
    let _ = run.stop();
}

#[test]
fn halka_open_island_returning_from_suppression_settles_at_once() {
    let run = halka_at_rest();
    let locked = Scene {
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
        ..expanded(0)
    };
    run.mailbox.publish(locked);
    run.assert_parked();
    let painted = run.frames().len();
    run.mailbox.publish(expanded(0));
    run.wait_frames(painted + 1);
    run.assert_parked();
    assert_eq!(run.frames().len(), painted + 1, "one frame, no motion");
    assert!(run.reports.try_recv().is_err(), "no motion report");
    assert_layers(
        last(&run.frames()),
        MaterialLayers::OPEN,
        "open after the snap",
    );
    let _ = run.stop();
}
