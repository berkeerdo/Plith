use super::*;

#[test]
fn fatal_beats_watchdog_beats_idle_beats_loop() {
    assert_eq!(exit_code(0, true, true, Some(true)), EXIT_RENDER_FATAL);
    assert_eq!(exit_code(0, false, true, Some(true)), EXIT_WATCHDOG);
    assert_eq!(exit_code(0, false, false, Some(false)), 1);
    assert_eq!(exit_code(0, false, false, Some(true)), 0);
    assert_eq!(exit_code(-1, false, false, None), -1);
}

#[test]
fn mode_text_names_the_mode() {
    assert_eq!(mode_text(&Launch::default()), "normal");
    let idle = Launch {
        idle_check_secs: Some(60),
        ..Launch::default()
    };
    assert_eq!(mode_text(&idle), "idle-check 60");
}
