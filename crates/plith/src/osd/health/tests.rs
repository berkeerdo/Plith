use super::{Failure, Health, HelperState, Notes, STABLE_UP, Step, failure_line, give_up_line};
use plith_core::osd::policy::{DOWN_GRACE, GIVE_UP_AFTER, LAUNCH_SPACING};
use std::time::{Duration, Instant};

const SECOND: Duration = Duration::from_secs(1);

/// Ticks once and, when a launch is asked, reports it as `ok`.
fn tick_launch(health: &mut Health, now: Instant, connected: bool, ok: bool) -> Step {
    let step = health.tick(now, connected, HelperState::Absent);
    if step.launch {
        health.launched(now, ok);
    }
    step
}

#[test]
fn the_first_tick_asks_for_a_launch() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    let step = health.tick(t0, false, HelperState::Absent);
    assert!(step.launch);
    assert_eq!(step.failure, None);
}

#[test]
fn no_launch_while_a_started_helper_is_alive_and_connecting() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    assert!(health.tick(t0, false, HelperState::Absent).launch);
    assert!(!health.launched(t0, true));
    let step = health.tick(t0 + SECOND, false, HelperState::Alive);
    assert_eq!(step, Step::default());
}

#[test]
fn a_new_helper_gets_the_whole_grace_to_connect() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    // Down long before the launch: the grace starts at the launch, not at the start.
    let late = t0 + Duration::from_secs(30);
    assert!(health.tick(late, false, HelperState::Absent).launch);
    health.launched(late, true);
    let before = late + DOWN_GRACE.saturating_sub(SECOND);
    assert_eq!(health.tick(before, false, HelperState::Alive).failure, None);
    let after = late + DOWN_GRACE;
    assert_eq!(
        health.tick(after, false, HelperState::Alive).failure,
        Some(Failure::Down)
    );
}

#[test]
fn an_exited_helper_is_a_failure_and_the_next_launch_waits_the_spacing() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    tick_launch(&mut health, t0, false, true);
    let step = health.tick(t0 + SECOND, false, HelperState::Exited);
    assert_eq!(step.failure, Some(Failure::Exited));
    assert!(!step.gave_up);
    let early = t0 + SECOND + LAUNCH_SPACING.saturating_sub(Duration::from_millis(1));
    assert!(!health.tick(early, false, HelperState::Absent).launch);
    assert!(
        health
            .tick(t0 + SECOND + LAUNCH_SPACING, false, HelperState::Absent)
            .launch
    );
}

#[test]
fn a_failed_launch_counts_and_repeated_ones_give_up() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    let mut now = t0;
    let mut launches = 0;
    for _ in 0..100 {
        let step = health.tick(now, false, HelperState::Absent);
        if step.launch {
            launches += 1;
            if health.launched(now, false) {
                break;
            }
        }
        now += SECOND;
    }
    assert_eq!(launches, GIVE_UP_AFTER);
    assert!(health.gave_up());
    // After giving up nothing is launched again.
    for _ in 0..30 {
        now += SECOND;
        assert!(!health.tick(now, false, HelperState::Absent).launch);
    }
}

#[test]
fn a_helper_that_connects_and_dies_at_once_still_gives_up() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    let mut now = t0;
    let mut gave_up = false;
    for _ in 0..200 {
        let step = health.tick(now, false, HelperState::Absent);
        if step.launch {
            health.launched(now, true);
            // Connected for one second, then the helper is gone.
            health.tick(now + SECOND, true, HelperState::Alive);
            let step = health.tick(now + 2 * SECOND, false, HelperState::Exited);
            if step.gave_up {
                gave_up = true;
                break;
            }
            now += 2 * SECOND;
        }
        now += SECOND;
    }
    assert!(gave_up);
}

#[test]
fn a_link_up_for_the_stable_time_clears_the_failure_history() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    let mut now = t0;
    // Four failed launches: one short of giving up.
    let mut failed = 0;
    while failed < GIVE_UP_AFTER - 1 {
        if health.tick(now, false, HelperState::Absent).launch {
            health.launched(now, false);
            failed += 1;
        }
        now += SECOND;
    }
    // The fifth launch works and the link stays up for the stable time.
    while !tick_launch(&mut health, now, false, true).launch {
        now += SECOND;
    }
    let up = now + SECOND;
    assert_eq!(health.tick(up, true, HelperState::Alive).link, Some(true));
    health.tick(up + STABLE_UP, true, HelperState::Alive);
    // Now one exit is a single failure, not the give-up.
    let step = health.tick(up + STABLE_UP + SECOND, false, HelperState::Exited);
    assert_eq!(step.failure, Some(Failure::Exited));
    assert!(!step.gave_up);
}

#[test]
fn a_due_launch_is_skipped_when_the_link_is_already_up() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    let step = health.tick(t0, true, HelperState::Absent);
    assert!(!step.launch);
    assert_eq!(step.link, Some(true));
    // Watching now: losing the link for the grace counts as a failure.
    health.tick(t0 + SECOND, false, HelperState::Absent);
    let step = health.tick(t0 + SECOND + DOWN_GRACE, false, HelperState::Absent);
    assert_eq!(step.failure, Some(Failure::Down));
}

#[test]
fn link_changes_are_reported_once_each() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    tick_launch(&mut health, t0, false, true);
    assert_eq!(
        health.tick(t0 + SECOND, true, HelperState::Alive).link,
        Some(true)
    );
    assert_eq!(
        health.tick(t0 + 2 * SECOND, true, HelperState::Alive).link,
        None
    );
    assert_eq!(
        health.tick(t0 + 3 * SECOND, false, HelperState::Alive).link,
        Some(false)
    );
    assert_eq!(
        health.tick(t0 + 4 * SECOND, false, HelperState::Alive).link,
        None
    );
}

#[test]
fn the_link_is_not_trusted_after_our_helper_exits_until_it_goes_down() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    tick_launch(&mut health, t0, false, true);
    health.tick(t0 + SECOND, true, HelperState::Alive);
    assert!(health.link_up());
    // The helper is gone but the link has not written since: it still looks connected.
    health.tick(t0 + 2 * SECOND, true, HelperState::Exited);
    assert!(!health.link_up());
    health.tick(t0 + 3 * SECOND, true, HelperState::Absent);
    assert!(!health.link_up());
    // Down, then a fresh connect: trusted again.
    health.tick(t0 + 4 * SECOND, false, HelperState::Absent);
    health.tick(t0 + 5 * SECOND, true, HelperState::Absent);
    assert!(health.link_up());
}

#[test]
fn a_stale_link_earns_no_credit() {
    let t0 = Instant::now();
    let mut health = Health::new(t0);
    let mut now = t0;
    let mut gave_up = false;
    for _ in 0..400 {
        if health.tick(now, false, HelperState::Absent).launch {
            health.launched(now, true);
            health.tick(now + SECOND, true, HelperState::Alive);
            let step = health.tick(now + 2 * SECOND, true, HelperState::Exited);
            gave_up |= step.gave_up;
            // The stale link lingers past the stable time before the keepalive notices.
            health.tick(now + SECOND + STABLE_UP, true, HelperState::Absent);
            now += 2 * SECOND + STABLE_UP;
            health.tick(now, false, HelperState::Absent);
        }
        if gave_up {
            break;
        }
        now += SECOND;
    }
    assert!(gave_up);
}

#[test]
fn notes_say_each_text_once_until_cleared() {
    let mut notes = Notes::default();
    assert!(notes.fresh("open: missing"));
    assert!(!notes.fresh("open: missing"));
    assert!(notes.fresh("write: broken"));
    assert!(!notes.fresh("open: missing"));
    notes.clear();
    assert!(notes.fresh("open: missing"));
}

#[test]
fn failure_lines_name_the_cause_and_the_detail() {
    assert_eq!(
        failure_line(Failure::Launch, Some("helper not installed")),
        "osd: helper launch failed: helper not installed"
    );
    assert_eq!(
        failure_line(Failure::Exited, None),
        "osd: helper exited (link: no error reported)"
    );
    assert_eq!(
        failure_line(Failure::Down, Some("open: The system cannot find the file")),
        "osd: link down for 3 s (link: open: The system cannot find the file)"
    );
    assert_eq!(
        give_up_line(),
        "osd: gave up after 5 failures within 60 s; a helper started by hand still connects"
    );
}
