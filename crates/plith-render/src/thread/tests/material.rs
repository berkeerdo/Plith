//! The material timeline inside the loop: a Halka expand runs until its ripples end, a plain
//! theme settles as it did before materials existed, interrupts and page switches move on
//! without a jump, and the motion line names the theme.

use super::expanded;
use super::harness::*;
use super::palette::handle_of;
use crate::paint::PaintFrame;
use plith_core::draw::{ArtId, ArtPixels};
use plith_core::material::art::{MaterialArt, ThemePaint};
use plith_core::material::{FADE_MS, HOLD_MS, MaterialLayers};
use plith_core::motion::MotionKind;
use plith_core::scene::{Scene, Suppression};
use plith_core::theme::Palette;
use plith_core::theme::registry::ThemeId;
use plith_core::tuning::BRISK;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod gate;

/// Frames of the fixed demo expand (rest to page 0, BRISK, 240 Hz) and of its collapse,
/// measured on the loop before the material timeline existed (tree at 497d9da).
const GECE_EXPAND_FRAMES: usize = 144;
const GECE_COLLAPSE_FRAMES: usize = 168;
/// The largest reveal or pill step between two frames that still reads as motion, not a jump.
const STEP: f64 = 0.15;

fn brisk_with(script: Vec<(u64, Scene)>, delay: Option<Duration>) -> Run {
    start(Setup {
        tune: Some(BRISK),
        script,
        delay,
        ..Setup::default()
    })
}

fn brisk() -> Run {
    brisk_with(Vec::new(), None)
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

/// Asserts that the reveal and the pill never step by more than `STEP` between frames.
fn assert_no_jump(frames: &[PaintFrame]) {
    for (i, pair) in frames.windows(2).enumerate() {
        let (a, b) = (pair[0].material, pair[1].material);
        let reveal = (a.reveal - b.reveal).abs();
        let pill = f64::from((a.pill - b.pill).abs());
        assert!(
            reveal < STEP && pill < STEP,
            "jump at frame {i}: {a:?} then {b:?}"
        );
    }
}

fn last(frames: &[PaintFrame]) -> MaterialLayers {
    frames.last().expect("a painted frame").material
}

/// `run` repainted at rest in Halka.
fn in_halka(run: Run) -> Run {
    run.wait_frames(1);
    handle_of(&run).set_theme(halka());
    run.wait_frames(2);
    run.assert_parked();
    run
}

/// A Halka run whose collapsed frame was repainted in Halka.
fn halka_at_rest() -> Run {
    in_halka(brisk())
}

/// Clock waits a Halka open from rest to page 0 takes (the fake clock is deterministic).
fn calls_to_open() -> u64 {
    let run = halka_at_rest();
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    let calls = run.harness.total();
    let _ = run.stop();
    calls
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
    assert!(open.to_string().ends_with("; theme halka"), "{open}");
    let (frames, calls) = (run.frames(), run.calls_at());
    let pressed = (before..frames.len())
        .find(|&i| frames[i].material.ripples.iter().all(|r| *r >= 1.0))
        .expect("all three ripples pressed at once");
    // The fake clock starts at 0 with the expand: call k is at k periods.
    let ms = |i: usize| f64::from(u32::try_from(calls[i]).unwrap()) * PERIOD * 1000.0;
    assert!(ms(pressed) < 200.0, "pressed while the shape opens");
    let end = ms(frames.len() - 1);
    assert!(
        end >= ms(pressed) + HOLD_MS + FADE_MS - PERIOD * 1000.0,
        "the motion ran past the hold and fade: {end} ms"
    );
    let motion = &frames[before..];
    for pair in motion[..pressed - before].windows(2) {
        let (a, b) = (pair[0].material.ripples, pair[1].material.ripples);
        assert!(a.iter().zip(b).all(|(x, y)| y >= *x), "a press came out");
    }
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
fn a_collapse_interrupting_an_expand_moves_on_from_where_it_is() {
    let run = in_halka(brisk_with(vec![(20, Scene::default())], None));
    let before = run.frames().len();
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.retargets, 1, "{report}");
    let frames = run.frames();
    assert!(
        frames[before..].iter().any(|f| f.material.ripples[0] > 0.0),
        "the expand had pressed a ripple before the collapse"
    );
    assert_no_jump(&frames[before..]);
    assert_layers(last(&frames), MaterialLayers::COLLAPSED, "collapsed");
    run.assert_parked();
    let _ = run.stop();
}

#[test]
fn a_reopen_during_a_collapse_moves_on_from_where_it_is() {
    // Collapse from page 0 (300 DIP), reopen 40 waits later onto page 2 (170 DIP).
    let reopen = calls_to_open() + 40;
    let run = in_halka(brisk_with(vec![(reopen, expanded(2))], None));
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    let before = run.frames().len();
    run.mailbox.publish(Scene::default());
    let report = run.report();
    assert_eq!(report.retargets, 1, "{report}");
    let frames = run.frames();
    assert_no_jump(&frames[before..]);
    assert_layers(last(&frames), MaterialLayers::OPEN, "open again");
    run.assert_parked();
    let _ = run.stop();
}

#[test]
fn a_page_switch_while_open_replays_nothing() {
    let run = halka_at_rest();
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    for page in [2, 0] {
        let before = run.frames().len();
        run.mailbox.publish(expanded(page));
        let report = run.report();
        assert_eq!(report.kind, MotionKind::Page, "{report}");
        for frame in &run.frames()[before..] {
            assert_layers(frame.material, MaterialLayers::OPEN, "page switch");
        }
    }
    run.assert_parked();
    let _ = run.stop();
}

#[test]
fn a_switch_to_gece_mid_motion_settles_and_parks() {
    let run = in_halka(brisk_with(Vec::new(), Some(Duration::from_millis(1))));
    run.mailbox.publish(expanded(0));
    run.wait_calls(10);
    handle_of(&run).set_theme(ThemePaint::plain(Palette::default()));
    let open = run.report();
    assert!(open.to_string().ends_with("; theme gece"), "{open}");
    run.assert_parked();
    let wakes = run.wakes();
    thread::sleep(QUIET);
    assert_eq!(run.wakes(), wakes, "no wake at rest");
    assert_eq!(last(&run.frames()), MaterialLayers::OPEN, "plain paint");
    let _ = run.stop();
}

#[test]
fn a_theme_switch_inside_the_ripple_window_still_repaints() {
    // A theme that lands right after a short collapse always shows its frame, whether or not
    // the material timeline had finished its ripple schedule (the loop is parked and has no
    // frame of its own).
    let run = brisk_with(Vec::new(), Some(Duration::from_millis(2)));
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(run.harness.total() + 3);
    run.mailbox.publish(Scene::default());
    let report = run.report();
    assert_eq!(report.retargets, 1, "{report}");
    run.assert_parked();
    let painted = run.frames().len();
    handle_of(&run).set_theme(halka());
    run.wait_frames(painted + 1);
    run.assert_parked();
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
