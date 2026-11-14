use super::*;
use crate::island::IslandMotion;
use crate::page::PageLayer;
use crate::shapes::{PAGES, REST};
use crate::tuning::R1;

const H: f64 = 1.0 / 240.0;

fn layer(offset: f64, opacity: f64) -> PageLayers {
    PageLayers::of(&[PageLayer {
        page: 0,
        offset,
        opacity,
    }])
}

fn state(shape: IslandShape, content: f64, pages: PageLayers) -> DrawnState {
    DrawnState {
        shape,
        content,
        pages,
    }
}

fn collapsed_rest() -> DrawnState {
    DrawnState::at_rest(REST, false, 0)
}

/// Collapses from the first page, optionally turning back to it at frame `turn`, and returns the
/// 1-based number of the last frame that differs from the rest state of the target in force.
fn visible_end(turn: Option<usize>) -> usize {
    let mut motion = IslandMotion::new(PAGES[0], R1);
    let mut target = REST;
    motion.retarget(target);
    let mut last = 0;
    for frame in 1..=1000 {
        if turn == Some(frame) {
            target = PAGES[0];
            motion.retarget(target);
        }
        motion.step(H);
        let expanded = target != REST;
        let rest = DrawnState::at_rest(target, expanded, 0);
        let drawn = state(
            motion.shape(),
            f64::from(u8::from(expanded)),
            layer(0.0, 1.0),
        );
        if visibly_differs(&drawn, &rest) {
            last = frame;
        }
    }
    last
}

#[test]
fn turnaround_moves_the_visible_end() {
    let plain = visible_end(None);
    let turned = visible_end(Some(60));
    assert!(plain > 0 && plain < 120, "plain visible end {plain}");
    assert!(turned > plain + 30, "turned {turned} vs plain {plain}");
    assert!(turned > 60, "visible end {turned} is before the turnaround");
}

#[test]
fn clamped_radius_tail_is_not_visible() {
    // A raw radius of 16.3 under height 32 is drawn clamped at 16: no difference.
    let mut motion = IslandMotion::new(PAGES[0], R1);
    motion.retarget(REST);
    let mut guard = 0;
    while !motion.step(H) {
        guard += 1;
        assert!(guard < 2000);
    }
    let drawn = state(motion.shape(), 0.0, layer(0.0, 1.0));
    assert!(!visibly_differs(&drawn, &collapsed_rest()));
    let clamped = IslandShape {
        radius: 16.0,
        ..REST
    };
    assert!(!visibly_differs(
        &state(clamped, 0.0, layer(0.0, 1.0)),
        &collapsed_rest()
    ));
}

#[test]
fn drift_is_compared_with_the_final_state_not_the_previous_frame() {
    let rest = collapsed_rest();
    let mut shape = REST;
    let mut seen = Vec::new();
    for _ in 0..4 {
        shape.width += 0.2;
        seen.push(visibly_differs(&state(shape, 0.0, layer(0.0, 1.0)), &rest));
    }
    // Each step is 0.2 DIP, below the threshold, yet the sum crosses 0.5 at the third.
    assert_eq!(seen, vec![false, false, true, true]);
}

#[test]
fn each_channel_has_its_own_threshold() {
    let rest = collapsed_rest();
    let differs = |s: DrawnState| visibly_differs(&s, &rest);
    assert!(!differs(state(REST, 0.001, layer(0.0, 1.0))));
    assert!(differs(state(REST, 0.01, layer(0.0, 1.0))));
    // Page layers only count while the content is visible, so compare against an open rest.
    let open = DrawnState::at_rest(REST, true, 0);
    let differs_open = |s: DrawnState| visibly_differs(&s, &open);
    assert!(!differs_open(state(REST, 1.0, layer(0.4, 1.0))));
    assert!(differs_open(state(REST, 1.0, layer(0.6, 1.0))));
    assert!(!differs_open(state(REST, 1.0, layer(0.0, 0.999))));
    assert!(differs_open(state(REST, 1.0, layer(0.0, 0.99))));
}

#[test]
fn an_extra_layer_differs_only_while_it_is_visible() {
    let rest = DrawnState::at_rest(REST, true, 0);
    let outgoing = |opacity| PageLayer {
        page: 1,
        offset: -5.0,
        opacity,
    };
    let incoming = PageLayer {
        page: 0,
        offset: 0.0,
        opacity: 1.0,
    };
    let faint = PageLayers::of(&[outgoing(0.001), incoming]);
    let clear = PageLayers::of(&[outgoing(0.2), incoming]);
    assert!(!visibly_differs(&state(REST, 1.0, faint), &rest));
    assert!(visibly_differs(&state(REST, 1.0, clear), &rest));
}

#[test]
fn pages_do_not_count_while_content_is_hidden() {
    let a = state(REST, 0.0, layer(0.0, 1.0));
    let b = state(REST, 0.0, layer(20.0, 0.3));
    assert!(!visibly_differs(&a, &b), "hidden content hides the pages");
    let a = state(REST, 1.0, layer(0.0, 1.0));
    let b = state(REST, 1.0, layer(20.0, 0.3));
    assert!(visibly_differs(&a, &b), "visible content shows the pages");
}
