use super::*;

const RETRY: TopmostAction = TopmostAction::Raise {
    retry_in_ms: Some(RETRY_MS),
    immediate: true,
};

#[test]
fn foreground_change_raises_now_and_schedules_four_retries() {
    let mut fight = TopmostFight::default();
    assert_eq!(fight.on_foreground(0), RETRY);
    for _ in 0..3 {
        assert_eq!(fight.on_timer(), RETRY);
    }
    assert_eq!(
        fight.on_timer(),
        TopmostAction::Raise {
            retry_in_ms: None,
            immediate: true
        }
    );
}

#[test]
fn retries_stop_after_four() {
    let mut fight = TopmostFight::default();
    fight.on_foreground(0);
    for _ in 0..RETRIES {
        assert!(matches!(fight.on_timer(), TopmostAction::Raise { .. }));
    }
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
}

#[test]
fn a_new_foreground_change_restarts_the_count() {
    let mut fight = TopmostFight::default();
    fight.on_foreground(0);
    fight.on_timer();
    fight.on_timer();
    fight.on_foreground(1_000);
    for _ in 0..RETRIES {
        assert!(matches!(fight.on_timer(), TopmostAction::Raise { .. }));
    }
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
}

#[test]
fn a_fresh_fight_is_idle() {
    assert_eq!(TopmostFight::default().on_timer(), TopmostAction::Idle);
}

#[test]
fn second_foreground_within_100_ms_does_not_raise_now() {
    let mut fight = TopmostFight::default();
    assert_eq!(fight.on_foreground(1_000), RETRY);
    // 99 ms in: one millisecond of the window is left.
    assert_eq!(
        fight.on_foreground(1_000 + DEBOUNCE_MS - 1),
        TopmostAction::Raise {
            retry_in_ms: Some(1),
            immediate: false
        }
    );
}

#[test]
fn debounced_change_raises_at_the_end_of_the_window() {
    let mut fight = TopmostFight::default();
    assert_eq!(fight.on_foreground(0), RETRY);
    assert_eq!(
        fight.on_foreground(30),
        TopmostAction::Raise {
            retry_in_ms: Some(70),
            immediate: false
        }
    );
    // The timer tick is the trailing-edge raise; retries then continue at the usual delay.
    assert_eq!(fight.on_timer(), RETRY);
}

#[test]
fn foreground_after_100_ms_raises() {
    let mut fight = TopmostFight::default();
    assert_eq!(fight.on_foreground(1_000), RETRY);
    assert_eq!(fight.on_foreground(1_000 + DEBOUNCE_MS), RETRY);
}

#[test]
fn a_debounced_change_still_restarts_the_retry_count() {
    let mut fight = TopmostFight::default();
    fight.on_foreground(0);
    fight.on_timer();
    fight.on_timer();
    fight.on_foreground(10);
    for _ in 0..RETRIES {
        assert!(matches!(fight.on_timer(), TopmostAction::Raise { .. }));
    }
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
}

#[test]
fn a_debounced_change_does_not_extend_the_window() {
    let mut fight = TopmostFight::default();
    fight.on_foreground(0);
    fight.on_foreground(60);
    // 100 ms after the raise, not after the swallowed event.
    assert_eq!(fight.on_foreground(100), RETRY);
}
