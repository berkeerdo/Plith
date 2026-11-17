use super::*;

const D: Day = (2026, 11, 17);
const A: Option<(&str, &str)> = Some(("Song A", "Artist"));
const B: Option<(&str, &str)> = Some(("Song B", "Artist"));

fn spans(log: &PlayLog) -> Vec<(u32, u32)> {
    log.in_window(-10_000, 10_000)
        .iter()
        .map(|p| (p.start_min, p.end_min))
        .collect()
}

#[test]
fn a_track_played_then_paused_is_one_interval() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    assert!(log.current().is_some());
    log.observe(D, 603, 0, A, true);
    log.observe(D, 605, 0, A, false);
    assert!(log.current().is_none());
    assert_eq!(spans(&log), vec![(600, 605)]);
}

#[test]
fn pause_and_resume_same_track_within_a_minute_merges() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 605, 0, A, false);
    log.observe(D, 605, 30, A, true);
    log.observe(D, 608, 0, A, false);
    assert_eq!(spans(&log), vec![(600, 608)]);
}

#[test]
fn resume_after_more_than_a_minute_opens_a_new_interval() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 605, 0, A, false);
    log.observe(D, 606, 5, A, true);
    log.observe(D, 608, 0, A, false);
    assert_eq!(spans(&log), vec![(600, 605), (606, 608)]);
}

#[test]
fn skip_under_ten_seconds_is_dropped() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 600, 5, B, true);
    log.observe(D, 603, 5, B, false);
    assert_eq!(spans(&log), vec![(600, 604)]);
    assert_eq!(log.in_window(0, 1440)[0].title, "Song B");
}

#[test]
fn track_change_closes_and_opens() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 603, 0, B, true);
    assert_eq!(log.current().unwrap().title, "Song B");
    log.observe(D, 606, 0, B, true);
    assert_eq!(log.current().unwrap().end_min, 606);
    assert_eq!(spans(&log), vec![(600, 603), (603, 606)]);
}

#[test]
fn track_becoming_none_closes_the_interval() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 602, 0, None, true);
    assert!(log.current().is_none());
    assert_eq!(spans(&log), vec![(600, 602)]);
}

#[test]
fn day_change_clears_yesterday() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 605, 0, A, false);
    log.observe(D, 1430, 0, B, true);
    log.observe((2026, 11, 18), 5, 0, B, true);
    assert_eq!(spans(&log), vec![(0, 5)]);
    assert_eq!(log.current().unwrap().start_min, 0);
}

#[test]
fn day_change_with_nothing_playing_leaves_an_empty_log() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, A, true);
    log.observe(D, 605, 0, A, false);
    log.observe((2026, 11, 18), 5, 0, None, false);
    assert!(spans(&log).is_empty());
}

#[test]
fn round_trip_with_tab_in_title_is_sanitised() {
    let mut log = PlayLog::default();
    log.observe(D, 600, 0, Some(("Tab\there", "Art\r\nist")), true);
    log.observe(D, 610, 0, Some(("Tab\there", "Art\r\nist")), false);
    log.observe(D, 700, 0, B, true);
    let text = log.serialize();
    assert!(text.starts_with("2026-11-17 600 610 Tab here\tArt  ist\n"));
    let back = PlayLog::parse(&text);
    assert!(back.current().is_none());
    assert_eq!(back.in_window(0, 1440).len(), 2);
    assert_eq!(back.in_window(0, 1440)[0].title, "Tab here");
    assert_eq!(back.in_window(0, 1440)[1].title, "Song B");
}

#[test]
fn parse_skips_malformed_lines_and_keeps_the_latest_day() {
    let text = "2026-11-16 10 20 Old\tX\ngarbage\n2026-11-17 30 40 New\tY\r\n2026-11-17 x 5 Bad\tY\n2026-11-17 50 40 Back\tY\n";
    let log = PlayLog::parse(text);
    assert_eq!(spans(&log), vec![(30, 40)]);
}

#[test]
fn in_window_handles_negative_from() {
    let mut log = PlayLog::default();
    log.observe(D, 0, 0, A, true);
    log.observe(D, 30, 0, A, false);
    assert_eq!(log.in_window(-120, 10).len(), 1);
    assert_eq!(log.in_window(-120, 0).len(), 0);
    assert_eq!(log.in_window(30, 1600).len(), 0);
    assert_eq!(log.in_window(29, 1600).len(), 1);
}
