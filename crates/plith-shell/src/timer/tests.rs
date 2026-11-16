use super::*;

#[test]
fn app_timer_ids_start_above_the_shell_ids() {
    assert!(!is_app_timer(0));
    assert!(!is_app_timer(1), "the topmost retry timer is the shell's");
    assert!(!is_app_timer(FIRST_APP_TIMER - 1));
    assert!(is_app_timer(FIRST_APP_TIMER));
    assert!(is_app_timer(FIRST_APP_TIMER + 3));
}
