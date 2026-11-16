//! Unit tests for the hidden-window journal: the pure text format, the boot stamp and the file
//! helpers, which run in a scratch folder under the temp directory that is removed afterwards.

use super::{
    BOOT_TOLERANCE_SECS, boot_stamp, clear, format, handles_for_boot, lock_name, parse, read,
    restore_orphans, write,
};
use crate::test_support::ScratchDir;

const BOOT: u64 = 1_800_000_000;

#[test]
fn round_trips_handles() {
    let handles = [0x1234_isize, -2, 0, isize::MAX];
    let text = format(BOOT, &handles);
    assert_eq!(text, format!("boot {BOOT}\n4660\n-2\n0\n{}\n", isize::MAX));
    assert_eq!(parse(&text), (Some(BOOT), handles.to_vec()));
}

#[test]
fn skips_garbage_lines() {
    let text = "boot 5\n12\nabc\n\n  34  \n-5x\n0x10\r\nboot 6\n56\r\n";
    assert_eq!(parse(text), (Some(5), vec![12, 34, 56]));
}

#[test]
fn empty_text_is_empty() {
    assert_eq!(parse(""), (None, Vec::new()));
    assert_eq!(parse("\n\n"), (None, Vec::new()));
    assert_eq!(format(BOOT, &[]), format!("boot {BOOT}\n"));
}

#[test]
fn stamped_round_trip_keeps_the_handles() {
    assert_eq!(handles_for_boot(&format(BOOT, &[7, 9]), BOOT), vec![7, 9]);
}

#[test]
fn missing_stamp_discards() {
    assert!(handles_for_boot("7\n9\n", BOOT).is_empty());
    assert!(handles_for_boot("boot x\n7\n", BOOT).is_empty());
}

#[test]
fn other_boot_discards() {
    let text = format(BOOT, &[7]);
    assert!(handles_for_boot(&text, BOOT + BOOT_TOLERANCE_SECS + 1).is_empty());
    assert!(handles_for_boot(&text, BOOT - BOOT_TOLERANCE_SECS - 1).is_empty());
}

#[test]
fn same_boot_within_tolerance_keeps() {
    let text = format(BOOT, &[7]);
    assert_eq!(handles_for_boot(&text, BOOT + BOOT_TOLERANCE_SECS), vec![7]);
    assert_eq!(handles_for_boot(&text, BOOT - BOOT_TOLERANCE_SECS), vec![7]);
}

#[test]
fn boot_stamp_is_now_minus_uptime() {
    assert_eq!(boot_stamp(1_000, 250_999), 750);
    assert_eq!(boot_stamp(10, 60_000), 0, "never below zero");
}

#[test]
fn write_read_and_clear_through_the_file() {
    let scratch = ScratchDir::new("journal-write");
    let path = scratch.journal();
    assert!(
        read(path, BOOT).is_empty(),
        "a missing journal reads as empty"
    );
    write(path, BOOT, &[7, 9]).expect("write");
    assert_eq!(read(path, BOOT), vec![7, 9]);
    assert!(
        read(path, BOOT + 3600).is_empty(),
        "another boot reads as empty"
    );
    assert!(
        !path.with_extension("tmp").exists(),
        "the temp file was renamed into place"
    );
    write(path, BOOT, &[9]).expect("rewrite");
    assert_eq!(read(path, BOOT), vec![9]);
    clear(path).expect("clear");
    assert!(!path.exists());
    clear(path).expect("clearing a missing journal is fine");
    write(path, BOOT, &[]).expect("an empty write");
    assert!(!path.exists(), "an empty set leaves no file");
}

#[test]
fn restore_orphans_shows_only_orphans_then_clears() {
    let scratch = ScratchDir::new("journal-orphans");
    let path = scratch.journal();
    write(path, BOOT, &[3, 4, 5]).expect("write");
    let mut shown = Vec::new();
    let count = restore_orphans(path, BOOT, |h| h != 4, |h| shown.push(h));
    assert_eq!(count, 2);
    assert_eq!(
        shown,
        vec![3, 5],
        "a recycled or visible handle is left alone"
    );
    assert!(!path.exists(), "the journal is cleared after the restore");
    assert_eq!(
        restore_orphans(path, BOOT, |_| true, |_| panic!("nothing left")),
        0
    );
}

#[test]
fn restore_orphans_from_another_boot_shows_nothing_and_clears() {
    let scratch = ScratchDir::new("journal-reboot");
    let path = scratch.journal();
    write(path, BOOT, &[3]).expect("write");
    let count = restore_orphans(path, BOOT + 86_400, |_| true, |_| panic!("other boot"));
    assert_eq!(count, 0);
    assert!(!path.exists(), "a stale journal is still cleared");
}

#[test]
fn journal_lock_name_is_stable_and_case_blind() {
    let path = std::path::Path::new(r"C:\Users\me\AppData\Local\Plith\flyout-hidden.txt");
    let name = lock_name(path);
    assert!(name.starts_with(r"Local\plith-flyout-"), "{name}");
    assert_eq!(name.len(), r"Local\plith-flyout-".len() + 16);
    assert_eq!(name, lock_name(path), "stable");
    let upper = std::path::Path::new(r"C:\USERS\ME\APPDATA\LOCAL\PLITH\FLYOUT-HIDDEN.TXT");
    assert_eq!(name, lock_name(upper), "case blind");
    let other = std::path::Path::new(r"C:\Temp\flyout-hidden.txt");
    assert_ne!(name, lock_name(other));
}
