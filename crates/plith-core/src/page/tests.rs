use super::*;
use crate::tuning::R1;

const H: f64 = 1.0 / 240.0;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn run_for(turn: &mut PageTurn, seconds: f64) {
    let mut t = 0.0;
    while t < seconds {
        turn.step(H);
        t += H;
    }
}

#[test]
fn turn_forward_slides_in_from_the_right_and_fades_in() {
    let mut turn = PageTurn::new(0, R1);
    turn.turn_to(1);
    turn.step(H);
    let layers = turn.layers();
    assert_eq!(layers.len(), 2);
    let incoming = layers[1];
    assert_eq!(incoming.page, 1);
    assert!(incoming.offset > 0.0, "offset {}", incoming.offset);
    assert!(incoming.opacity < 0.2, "opacity {}", incoming.opacity);
    assert_eq!(layers[0].page, 0);
}

#[test]
fn turn_backward_slides_in_from_the_left() {
    let mut turn = PageTurn::new(2, R1);
    turn.turn_to(1);
    turn.step(H);
    let layers = turn.layers();
    assert!(layers[1].offset < 0.0);
    assert!(layers[0].offset > 0.0 || layers[0].offset.abs() < PAGE_SLIDE);
}

#[test]
fn outgoing_layer_is_dropped_once_invisible() {
    let mut turn = PageTurn::new(0, R1);
    turn.turn_to(1);
    run_for(&mut turn, 2.0);
    let layers = turn.layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].page, 1);
    assert!(close(layers[0].offset, 0.0));
    assert!(close(layers[0].opacity, 1.0));
}

#[test]
fn reversal_mid_turn_keeps_velocity() {
    let mut turn = PageTurn::new(0, R1);
    turn.turn_to(1);
    run_for(&mut turn, 0.1);
    let before = turn.layers();
    let (out_before, in_before) = (before[0], before[1]);
    assert_eq!((out_before.page, in_before.page), (0, 1));
    let in_velocity = turn.incoming.offset.velocity();

    turn.turn_to(0);
    let after = turn.layers();
    assert_eq!(after.len(), 2);
    assert_eq!((after[0].page, after[1].page), (1, 0));
    // No jump: both layers keep value, and page 1 keeps its velocity.
    assert!(close(after[1].offset, out_before.offset));
    assert!(close(after[1].opacity, out_before.opacity));
    assert!(close(after[0].offset, in_before.offset));
    assert!(close(after[0].opacity, in_before.opacity));
    let outgoing = turn.outgoing.as_ref().expect("outgoing layer");
    assert!(close(outgoing.offset.velocity(), in_velocity));
    assert!(close(outgoing.offset.target(), PAGE_SLIDE));
    assert!(close(turn.incoming.offset.target(), 0.0));
    assert_eq!(turn.current(), 0);
}

#[test]
fn turn_to_third_page_makes_incoming_outgoing_and_drops_older() {
    let mut turn = PageTurn::new(0, R1);
    turn.turn_to(1);
    run_for(&mut turn, 0.05);
    let mid = turn.layers()[1];
    turn.turn_to(2);
    let layers = turn.layers();
    assert_eq!(layers.len(), 2);
    assert_eq!((layers[0].page, layers[1].page), (1, 2));
    assert!(close(layers[0].offset, mid.offset));
    assert!(close(layers[0].opacity, mid.opacity));
    assert!(close(layers[1].offset, PAGE_SLIDE));
    assert!(close(layers[1].opacity, 0.0));
}

#[test]
fn turn_settles_under_0_6_s() {
    let mut turn = PageTurn::new(0, R1);
    turn.turn_to(1);
    let mut t = 0.0;
    let mut settled = false;
    while t < 0.6 && !settled {
        settled = turn.step(H);
        t += H;
    }
    assert!(settled, "not settled after {t} s");
}

#[test]
fn turn_to_current_page_is_a_no_op() {
    let mut turn = PageTurn::new(3, R1);
    turn.turn_to(3);
    assert_eq!(turn.layers().len(), 1);
    assert!(turn.step(H));
    assert_eq!(turn.current(), 3);
}

#[test]
fn layers_hold_outgoing_then_incoming_without_allocating() {
    let mut turn = PageTurn::new(0, R1);
    turn.turn_to(1);
    let pages: Vec<usize> = turn.layers().iter().map(|l| l.page).collect();
    assert_eq!(pages, vec![0, 1]);
}

#[test]
fn a_motion_shows_its_start_page_and_its_target_page() {
    assert_eq!(
        motion_pages(2, true, 2),
        vec![2],
        "an expand shows its page"
    );
    assert_eq!(motion_pages(0, true, 2), vec![0, 2], "a turn shows both");
    assert_eq!(
        motion_pages(1, true, 3),
        vec![1, 3],
        "an expand that turns fades the old page out"
    );
    assert!(
        motion_pages(1, false, 1).is_empty(),
        "a collapse shows the pill"
    );
    assert!(motion_pages(0, false, 2).is_empty());
}

#[test]
fn page_turn_welcome_to_media_and_back() {
    use crate::pages::PageId;
    let welcome = PageId::Welcome.index();
    let media = PageId::Media.index();
    let mut turn = PageTurn::new(welcome, R1);
    turn.turn_to(media);
    turn.step(H);
    let layers = turn.layers();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].page, welcome);
    assert_eq!(layers[1].page, media);
    run_for(&mut turn, 1.0);
    assert_eq!(turn.current(), media);
    turn.turn_to(welcome);
    run_for(&mut turn, 1.0);
    assert_eq!(turn.current(), welcome);
    assert_eq!(turn.layers().len(), 1);
}
