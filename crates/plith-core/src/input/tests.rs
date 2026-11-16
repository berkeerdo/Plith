use super::*;
use crate::draw::Rect;
use crate::pages::PageId;

fn target(x: f64, y: f64, w: f64, h: f64, action: Action) -> HitTarget {
    HitTarget {
        rect: Rect { x, y, w, h },
        action,
    }
}

/// Page 0 has a tab and a pick target; page 1 a play button at the same place as page 0's pick.
fn targets() -> [Vec<HitTarget>; 4] {
    [
        vec![
            target(10.0, 10.0, 40.0, 20.0, Action::Tab(PageId::Media)),
            target(100.0, 100.0, 40.0, 20.0, Action::PickSource(0)),
        ],
        vec![target(100.0, 100.0, 40.0, 20.0, Action::PlayPause)],
        Vec::new(),
        Vec::new(),
    ]
}

fn layer(page: usize, offset: f64, opacity: f64) -> PageLayer {
    PageLayer {
        page,
        offset,
        opacity,
    }
}

#[test]
fn click_on_pill_expands() {
    let layers = PageLayers::of(&[layer(0, 0.0, 1.0)]);
    assert_eq!(
        route_click(false, &layers, &targets(), 500.0, 500.0),
        ClickResult::Expand
    );
}

#[test]
fn click_while_collapsed_expands() {
    let layers = PageLayers::of(&[layer(0, 0.0, 1.0)]);
    assert_eq!(
        route_click(false, &layers, &targets(), 20.0, 20.0),
        ClickResult::Expand,
        "a collapsed island never hits a page target"
    );
    assert_eq!(
        route_click(false, &PageLayers::empty(), &targets(), 20.0, 20.0),
        ClickResult::Expand
    );
}

#[test]
fn click_on_a_target_returns_its_action() {
    let layers = PageLayers::of(&[layer(0, 0.0, 1.0)]);
    assert_eq!(
        route_click(true, &layers, &targets(), 20.0, 20.0),
        ClickResult::Action(Action::Tab(PageId::Media))
    );
    assert_eq!(
        route_click(true, &layers, &targets(), 500.0, 500.0),
        ClickResult::Nothing,
        "empty page space does not collapse"
    );
    assert_eq!(
        route_click(true, &PageLayers::empty(), &targets(), 20.0, 20.0),
        ClickResult::Nothing
    );
}

#[test]
fn click_during_turn_hits_incoming_page() {
    // Early in the turn the outgoing page 0 still owns the clicks.
    let early = PageLayers::of(&[layer(0, -5.0, 0.7), layer(1, 15.0, 0.3)]);
    assert_eq!(page_on_screen(&early).map(|l| l.page), Some(0));
    assert_eq!(
        route_click(true, &early, &targets(), 105.0, 110.0),
        ClickResult::Action(Action::PickSource(0)),
        "x is shifted by the outgoing offset: 105 - (-5) = 110"
    );
    // Past half opacity the incoming page 1 does, at its own offset.
    let late = PageLayers::of(&[layer(0, -15.0, 0.3), layer(1, 5.0, 0.5)]);
    assert_eq!(page_on_screen(&late).map(|l| l.page), Some(1));
    assert_eq!(
        route_click(true, &late, &targets(), 148.0, 110.0),
        ClickResult::Action(Action::PlayPause),
        "148 - 5 = 143 is inside 100..140 plus the 4 DIP slack"
    );
    assert_eq!(
        route_click(true, &late, &targets(), 155.0, 110.0),
        ClickResult::Nothing
    );
}

#[test]
fn click_on_empty_space_cancels_pick() {
    let picking = [
        vec![
            target(100.0, 100.0, 40.0, 20.0, Action::PickSource(0)),
            target(200.0, 80.0, 200.0, 200.0, Action::CancelPick),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ];
    let layers = PageLayers::of(&[layer(0, 0.0, 1.0)]);
    assert_eq!(
        route_click(true, &layers, &picking, 300.0, 150.0),
        ClickResult::Action(Action::CancelPick)
    );
    assert_eq!(toggle_pick(PickMode::None, 2), PickMode::Picked(2));
    assert_eq!(toggle_pick(PickMode::Picked(2), 2), PickMode::None);
    assert_eq!(toggle_pick(PickMode::Picked(2), 1), PickMode::Picked(1));
}

#[test]
fn leave_then_return_cancels_collapse() {
    let mut timer = CollapseTimer::default();
    assert!(!timer.due(10.0), "never left");
    timer.leave(1.0);
    assert!(!timer.due(1.39));
    assert!(timer.due(1.4));
    timer.enter();
    assert_eq!(timer.leave_at, None);
    assert!(!timer.due(5.0), "back inside");
    timer.leave(6.0);
    timer.leave(6.3);
    assert!(timer.due(6.4), "a second leave keeps the first time");
}

#[test]
fn remaining_counts_down_to_zero() {
    let mut timer = CollapseTimer::default();
    assert_eq!(timer.remaining(1.0), None);
    timer.leave(1.0);
    assert!((timer.remaining(1.1).unwrap() - 0.3).abs() < 1e-9);
    assert_eq!(timer.remaining(2.0), Some(0.0));
}

#[test]
fn a_pick_out_of_range_is_dropped() {
    assert_eq!(pick_within(PickMode::Picked(2), 3), PickMode::Picked(2));
    assert_eq!(pick_within(PickMode::Picked(3), 3), PickMode::None);
    assert_eq!(pick_within(PickMode::Picked(0), 0), PickMode::None);
    assert_eq!(pick_within(PickMode::None, 3), PickMode::None);
}

#[test]
fn a_route_needs_the_mixer_the_pick_was_made_in() {
    use crate::pages::fixtures;
    let banana = MixerShape::of(fixtures::banana().vm.as_ref());
    let potato = MixerShape::of(fixtures::potato().vm.as_ref());
    assert!(banana.is_some() && banana != potato);
    assert!(route_allowed(banana, banana));
    assert!(!route_allowed(banana, potato), "the mixer changed edition");
    assert!(!route_allowed(banana, None), "the mixer went away");
    assert!(!route_allowed(None, None), "no mixer, no route");
}
