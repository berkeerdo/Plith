use super::*;
use plith_ipc::{Message, Meter};

const FRAME: f64 = 1.0 / 240.0;

fn level(meter: Meter, percent: u8, muted: bool, label: &str) -> Message {
    Message::Level {
        meter,
        percent,
        muted,
        label: label.to_string(),
    }
}

fn hello() -> Message {
    Message::Hello { client_pid: 7 }
}

fn volume(percent: u8) -> Message {
    level(Meter::Volume, percent, false, "Speakers")
}

/// Steps from `from` (exclusive) to `to` (inclusive) at 240 Hz; returns the last `step` result.
fn run(model: &mut OsdModel, from: f64, to: f64) -> bool {
    let mut animating = false;
    let mut t = from;
    while t + FRAME <= to + 1e-9 {
        t += FRAME;
        animating = model.step(t);
    }
    animating
}

#[test]
fn level_shows_then_hides_after_1_5s() {
    let mut model = OsdModel::new();
    assert!(model.frame().opacity.abs() < f32::EPSILON, "starts hidden");
    model.apply(&hello(), 0.0);
    model.apply(&volume(40), 0.0);
    assert!(model.step(FRAME), "the show animates");
    assert!(!run(&mut model, FRAME, 1.0), "the show settles within 1 s");
    assert!((model.frame().opacity - 1.0).abs() < f32::EPSILON);
    assert!((model.frame().level - 0.4).abs() < 1e-6);
    assert!(!run(&mut model, 1.0, 1.49), "holds until 1.5 s");
    assert!((model.frame().opacity - 1.0).abs() < f32::EPSILON);
    assert!(model.step(1.5), "the hide starts at 1.5 s");
    assert!(!run(&mut model, 1.5, 2.5), "the hide settles");
    assert!(model.frame().opacity.abs() < f32::EPSILON);
    assert_eq!(model.deadline(), None, "nothing is due once hidden");
}

#[test]
fn new_level_rearms_the_timer() {
    let mut model = OsdModel::new();
    model.apply(&volume(40), 0.0);
    run(&mut model, 0.0, 1.0);
    model.apply(&volume(50), 1.0);
    assert_eq!(model.deadline(), Some(2.5));
    run(&mut model, 1.0, 2.45);
    assert!(
        (model.frame().opacity - 1.0).abs() < f32::EPSILON,
        "still shown at 2.45 s"
    );
    assert!(model.step(2.5), "the hide starts at 2.5 s");
}

#[test]
fn hide_starts_fading_at_once() {
    let mut model = OsdModel::new();
    model.apply(&hello(), 0.0);
    model.apply(&volume(40), 0.0);
    run(&mut model, 0.0, 0.5);
    model.apply(&Message::Hide, 0.5);
    assert!(!model.is_showing());
    assert_eq!(model.deadline(), None, "the hide timer is disarmed");
    assert!(model.step(0.5 + FRAME));
    assert!(model.step(0.5 + 2.0 * FRAME));
    assert!(model.frame().opacity < 1.0, "fading on the next frames");
}

#[test]
fn level_retarget_keeps_velocity() {
    let mut model = OsdModel::new();
    model.apply(&volume(20), 0.0);
    run(&mut model, 0.0, 0.5);
    assert!(
        (model.frame().level - 0.2).abs() < 1e-6,
        "a show from hidden starts at the level"
    );
    model.apply(&volume(80), 0.5);
    run(&mut model, 0.5, 0.5 + 10.0 * FRAME);
    let before = model.frame().level;
    assert!(before > 0.2 && before < 0.8, "mid flight: {before}");
    model.apply(&volume(30), 0.5 + 10.0 * FRAME);
    model.step(0.5 + 11.0 * FRAME);
    assert!(
        model.frame().level > before,
        "the upward velocity carries on"
    );
    let mut t = 0.5 + 11.0 * FRAME;
    let mut lowest = f32::MAX;
    while t < 1.5 {
        t += FRAME;
        model.step(t);
        lowest = lowest.min(model.frame().level);
    }
    assert!(lowest >= 0.3 - 1e-3, "no overshoot below 30 %: {lowest}");
    assert!((model.frame().level - 0.3).abs() < 1e-6);
}

#[test]
fn rest_reports_not_animating() {
    let mut model = OsdModel::new();
    assert!(!model.step(0.0), "a new model rests");
    assert_eq!(
        model.deadline(),
        Some(START_GRACE_S),
        "only the start grace is due"
    );
    model.apply(&hello(), 0.0);
    assert_eq!(model.deadline(), None);
    model.apply(&volume(40), 0.0);
    assert!(!run(&mut model, 0.0, 1.0));
    assert!(!model.step(1.1), "a settled show rests");
    assert_eq!(model.deadline(), Some(1.5));
}

#[test]
fn bye_and_lost_client_exit() {
    let mut model = OsdModel::new();
    model.apply(&Message::Hello { client_pid: 7 }, 0.0);
    assert!(!model.should_exit(0.0));
    model.apply(&Message::Bye, 1.0);
    assert!(model.should_exit(1.0), "Bye exits at once");

    let mut model = OsdModel::new();
    model.apply(&Message::Hello { client_pid: 7 }, 0.0);
    model.client_gone(5.0);
    assert_eq!(model.deadline(), Some(7.0));
    assert!(!model.should_exit(6.99));
    assert!(model.should_exit(7.0), "2 s without a client");

    let mut model = OsdModel::new();
    model.client_gone(5.0);
    model.apply(&Message::Hello { client_pid: 8 }, 6.0);
    assert!(!model.should_exit(8.0), "a reconnect disarms the exit");
}

#[test]
fn muted_frame_carries_the_flag() {
    let mut model = OsdModel::new();
    model.apply(&level(Meter::Volume, 35, true, "Headset"), 0.0);
    let frame = model.frame();
    assert!(frame.muted);
    assert_eq!(frame.meter, Meter::Volume);
    assert_eq!(frame.label, "Headset");
    assert!((frame.level - 0.35).abs() < 1e-6);
    model.apply(&level(Meter::Volume, 35, false, "Headset"), 0.1);
    assert!(!model.frame().muted);
}

#[test]
fn meter_change_snaps_the_level() {
    let mut model = OsdModel::new();
    model.apply(&volume(90), 0.0);
    run(&mut model, 0.0, 0.5);
    model.apply(&level(Meter::Brightness, 30, false, "Display"), 0.5);
    let frame = model.frame();
    assert_eq!(frame.meter, Meter::Brightness);
    assert!(
        (frame.level - 0.3).abs() < 1e-6,
        "no sweep from volume to brightness"
    );
}

#[test]
fn level_during_hide_shows_again() {
    let mut model = OsdModel::new();
    model.apply(&volume(40), 0.0);
    run(&mut model, 0.0, 0.5);
    model.apply(&Message::Hide, 0.5);
    run(&mut model, 0.5, 0.55);
    model.apply(&volume(45), 0.55);
    assert!(model.is_showing());
    assert!(!run(&mut model, 0.55, 1.5));
    assert!((model.frame().opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_change_without_motion_asks_for_one_frame() {
    let mut model = OsdModel::new();
    model.apply(&volume(40), 0.0);
    assert!(!run(&mut model, 0.0, 1.0));
    model.apply(&level(Meter::Volume, 40, true, "Speakers"), 1.0);
    assert!(model.step(1.0 + FRAME), "the mute flag needs a repaint");
    assert!(!model.step(1.0 + 2.0 * FRAME), "then it rests");
    model.apply(&level(Meter::Brightness, 70, false, "Display"), 1.1);
    assert!(model.step(1.1 + FRAME), "a meter switch snaps and repaints");
    assert!(!model.step(1.1 + 2.0 * FRAME));
}

#[test]
fn disconnect_then_2s_exits() {
    let mut model = OsdModel::new();
    model.apply(&hello(), 0.0);
    model.disconnected(3.0);
    assert_eq!(model.deadline(), Some(5.0));
    assert!(!model.should_exit(4.99));
    assert!(model.should_exit(5.0));
}

#[test]
fn reconnect_within_2s_stays() {
    let mut model = OsdModel::new();
    model.apply(&hello(), 0.0);
    model.disconnected(3.0);
    model.apply(&hello(), 4.0);
    assert_eq!(model.deadline(), None);
    assert!(!model.should_exit(6.0));
}

#[test]
fn no_hello_in_10s_exits() {
    let model = OsdModel::new();
    assert_eq!(model.deadline(), Some(10.0));
    assert!(!model.should_exit(9.99));
    assert!(model.should_exit(10.0));
    let mut model = OsdModel::new();
    model.apply(&hello(), 1.0);
    assert!(!model.should_exit(20.0), "a Hello ends the start grace");
}

#[test]
fn bye_fades_out_before_exit() {
    let mut model = OsdModel::new();
    model.apply(&hello(), 0.0);
    model.apply(&volume(40), 0.0);
    run(&mut model, 0.0, 0.5);
    model.apply(&Message::Bye, 0.5);
    assert!(!model.is_showing(), "Bye starts the fade");
    assert!(!model.should_exit(0.5), "not while visible");
    let mut t = 0.5;
    while !model.is_hidden() {
        t += FRAME;
        model.step(t);
        assert!(t < 1.0, "the fade ends within 0.5 s");
    }
    assert!(model.should_exit(t), "exit once hidden");
    model.apply(&volume(60), t);
    assert!(model.is_hidden(), "nothing shows after Bye");
}

#[test]
fn bye_exits_after_half_a_second_at_most() {
    let mut model = OsdModel::new();
    model.apply(&hello(), 0.0);
    model.apply(&volume(40), 0.0);
    run(&mut model, 0.0, 0.5);
    model.apply(&Message::Bye, 0.5);
    assert_eq!(model.deadline(), Some(1.0));
    assert!(!model.should_exit(0.99));
    assert!(model.should_exit(1.0), "even if no frame was stepped");
}

#[test]
fn only_opacity_moves_are_fading() {
    let mut model = OsdModel::new();
    model.apply(&volume(40), 0.0);
    assert!(model.is_fading(), "a show fades in");
    run(&mut model, 0.0, 0.5);
    assert!(!model.is_fading());
    model.apply(&volume(90), 0.5);
    assert!(model.step(0.5 + FRAME), "the level moves");
    assert!(!model.is_fading(), "but the opacity does not");
    model.apply(&Message::Hide, 0.6);
    assert!(model.is_fading(), "a hide fades out");
}
