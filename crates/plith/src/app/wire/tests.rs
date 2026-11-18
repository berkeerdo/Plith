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

fn temp_log(name: &str) -> (std::path::PathBuf, LogFile) {
    let dir = std::env::temp_dir().join(format!("plith-wire-{name}-{}", std::process::id()));
    let path = dir.join("plith.log");
    let log = LogFile::open_at(&path).unwrap();
    (dir, log)
}

#[test]
fn a_panic_while_closing_still_sets_fatal_and_logs_render_fatal() {
    let (dir, log) = temp_log("panic");
    let fatal = AtomicBool::new(false);
    let reason = Fatal::Panic(String::from("render thread panicked: boom"));
    assert!(handle_fatal(&reason, true, &log, &fatal));
    assert!(fatal.load(Ordering::Acquire));
    log.flush();
    let text = std::fs::read_to_string(dir.join("plith.log")).unwrap();
    assert!(
        text.contains("render fatal: render thread panicked: boom"),
        "{text}"
    );
    drop(log);
    std::fs::remove_file(dir.join("plith.log")).unwrap();
    std::fs::remove_dir(&dir).unwrap();
}

#[test]
fn a_plain_error_while_closing_is_a_clean_close() {
    let (dir, log) = temp_log("error");
    let fatal = AtomicBool::new(false);
    let reason = Fatal::Error(String::from("present failed"));
    assert!(!handle_fatal(&reason, true, &log, &fatal));
    assert!(!fatal.load(Ordering::Acquire));
    assert!(handle_fatal(&reason, false, &log, &fatal));
    assert!(fatal.load(Ordering::Acquire));
    drop(log);
    std::fs::remove_file(dir.join("plith.log")).unwrap();
    std::fs::remove_dir(&dir).unwrap();
}

#[test]
fn a_real_services_idle_check_fits_its_watchdog() {
    use crate::idle::{SETTLE_MS, START_CAP};
    // Worst case: the whole start cap, the settle, then the full measured window.
    let worst_ms = START_CAP.as_millis() + u128::from(SETTLE_MS) + 60_000;
    let budget = watchdog_secs(Some(60), true);
    assert!(
        u128::from(budget) * 1000 > worst_ms,
        "{budget} s must cover {worst_ms} ms"
    );
    assert_eq!(budget, 20 + 2 + 60 + WATCHDOG_MARGIN_SECS);
}

#[test]
fn a_long_fixture_idle_check_fits_its_watchdog() {
    use crate::idle::SETTLE_MS;
    // A fixture run has no start wait: the settle, then the full measured window.
    let worst_ms = u128::from(SETTLE_MS) + 120_000;
    let budget = watchdog_secs(Some(120), false);
    assert!(
        u128::from(budget) * 1000 > worst_ms,
        "{budget} s must cover {worst_ms} ms"
    );
    assert_eq!(budget, 2 + 120 + WATCHDOG_MARGIN_SECS);
}

#[test]
fn short_idle_checks_and_demos_keep_the_plain_watchdog() {
    assert_eq!(watchdog_secs(Some(60), false), WATCHDOG_SECS);
    assert_eq!(watchdog_secs(None, false), WATCHDOG_SECS);
    assert_eq!(watchdog_secs(None, true), WATCHDOG_SECS);
    assert_eq!(watchdog_secs(Some(5), true), WATCHDOG_SECS, "never shorter");
}
