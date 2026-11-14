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

#[test]
fn raised_line_names_foreground_and_culprit() {
    assert_eq!(
        raised_line("Chrome_WidgetWin_1", Some("Shell_TrayWnd"), true),
        "topmost raise (foreground: Chrome_WidgetWin_1; above: Shell_TrayWnd)"
    );
    assert_eq!(
        raised_line("Notepad", None, true),
        "topmost raise (foreground: Notepad; above: none)"
    );
}

#[test]
fn raised_line_for_a_debounced_change_says_retry_armed() {
    assert_eq!(
        raised_line("?", None, false),
        "topmost retry armed (foreground: ?)"
    );
}
