use super::*;

#[test]
fn line_has_padded_stamp_then_text() {
    let stamp = Stamp {
        year: 2026,
        month: 3,
        day: 7,
        hour: 4,
        minute: 5,
        second: 6,
        millis: 9,
    };
    assert_eq!(
        format_line(stamp, "spike start"),
        "2026-03-07 04:05:06.009 spike start\n"
    );
}

#[test]
fn a_log_without_a_file_ignores_lines() {
    let log = Log {
        file: Mutex::new(None),
    };
    log.line("dropped");
}
