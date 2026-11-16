use super::*;

fn stamp() -> Stamp {
    Stamp {
        year: 2026,
        month: 3,
        day: 7,
        hour: 4,
        minute: 5,
        second: 6,
        millis: 9,
    }
}

#[test]
fn line_has_padded_stamp_then_text() {
    assert_eq!(
        format_line(stamp(), "spike start"),
        "2026-03-07 04:05:06.009 spike start\n"
    );
}

#[test]
fn escape_controls_keeps_plain_text_borrowed() {
    assert!(matches!(
        escape_controls("osd start pipe=x İ"),
        Cow::Borrowed(_)
    ));
}

#[test]
fn escape_controls_turns_line_breaks_into_escapes() {
    assert_eq!(
        escape_controls("a\nb\rc\td\u{2028}e"),
        "a\\u{a}b\\u{d}c\\u{9}d\\u{2028}e"
    );
}

#[test]
fn a_forged_line_stays_one_line() {
    let line = format_line(stamp(), "pipe=x\n2026-01-01 00:00:00.000 osd done");
    assert_eq!(line.matches('\n').count(), 1, "{line}");
    assert!(line.ends_with('\n'));
}

#[test]
fn line_fate_writes_until_the_cap_then_one_notice_then_drops() {
    assert_eq!(line_fate(0, 100, false), LineFate::Write);
    assert_eq!(line_fate(LOG_CAP - 100, 100, false), LineFate::Write);
    assert_eq!(line_fate(LOG_CAP - 99, 100, false), LineFate::Notice);
    assert_eq!(line_fate(LOG_CAP - 99, 100, true), LineFate::Drop);
    assert_eq!(line_fate(0, 1, true), LineFate::Drop);
}

#[test]
fn only_an_oversized_old_log_rotates() {
    assert!(!rotate_at_open(0));
    assert!(!rotate_at_open(LOG_CAP));
    assert!(rotate_at_open(LOG_CAP + 1));
}

#[test]
fn same_final_path_ignores_prefix_and_case_only() {
    let expected = r"C:\Users\me\AppData\Local\Plith\plith-osd.log";
    assert!(same_final_path(
        r"\\?\C:\Users\ME\AppData\Local\Plith\plith-osd.log",
        expected
    ));
    assert!(same_final_path(expected, expected));
    assert!(!same_final_path(
        r"\\?\C:\Windows\System32\plith-osd.log",
        expected
    ));
    assert!(!same_final_path(
        r"\\?\C:\Users\me\AppData\Local\Other\plith-osd.log",
        expected
    ));
}
