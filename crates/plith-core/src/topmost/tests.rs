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
fn second_foreground_within_100_ms_only_rearms() {
    let mut fight = TopmostFight::default();
    assert_eq!(fight.on_foreground(1_000), RETRY);
    assert_eq!(
        fight.on_foreground(1_000 + DEBOUNCE_MS - 1),
        TopmostAction::Raise {
            retry_in_ms: Some(RETRY_MS),
            immediate: false
        }
    );
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
