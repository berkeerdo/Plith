use super::*;

#[test]
fn foreground_change_raises_now_and_schedules_four_retries() {
    let mut fight = TopmostFight::default();
    assert_eq!(
        fight.on_foreground(),
        TopmostAction::Raise {
            retry_in_ms: Some(RETRY_MS)
        }
    );
    for _ in 0..3 {
        assert_eq!(
            fight.on_timer(),
            TopmostAction::Raise {
                retry_in_ms: Some(RETRY_MS)
            }
        );
    }
    assert_eq!(fight.on_timer(), TopmostAction::Raise { retry_in_ms: None });
}

#[test]
fn retries_stop_after_four() {
    let mut fight = TopmostFight::default();
    fight.on_foreground();
    for _ in 0..RETRIES {
        assert!(matches!(fight.on_timer(), TopmostAction::Raise { .. }));
    }
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
}

#[test]
fn a_new_foreground_change_restarts_the_count() {
    let mut fight = TopmostFight::default();
    fight.on_foreground();
    fight.on_timer();
    fight.on_timer();
    fight.on_foreground();
    for _ in 0..RETRIES {
        assert!(matches!(fight.on_timer(), TopmostAction::Raise { .. }));
    }
    assert_eq!(fight.on_timer(), TopmostAction::Idle);
}

#[test]
fn a_fresh_fight_is_idle() {
    assert_eq!(TopmostFight::default().on_timer(), TopmostAction::Idle);
}
