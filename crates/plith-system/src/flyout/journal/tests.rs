//! Unit tests for the hidden-window journal: the pure text format and the file helpers, which
//! run against a fresh folder under the temp directory.

use std::path::PathBuf;

use super::{clear, format, parse, read, restore_orphans, write};

/// A journal path in a folder unique to this test (and this process), created empty.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("plith-journal-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp folder");
    let path = dir.join("flyout-hidden.txt");
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn round_trips_handles() {
    let handles = [0x1234_isize, -2, 0, isize::MAX];
    let text = format(&handles);
    assert_eq!(text, format!("4660\n-2\n0\n{}\n", isize::MAX));
    assert_eq!(parse(&text), handles);
}

#[test]
fn skips_garbage_lines() {
    let text = "12\nabc\n\n  34  \n-5x\n0x10\r\n56\r\n";
    assert_eq!(parse(text), vec![12, 34, 56]);
}

#[test]
fn empty_text_is_empty() {
    assert!(parse("").is_empty());
    assert!(parse("\n\n").is_empty());
    assert_eq!(format(&[]), "");
}

#[test]
fn write_read_and_clear_through_the_file() {
    let path = scratch("write");
    assert!(read(&path).is_empty(), "a missing journal reads as empty");
    write(&path, &[7, 9]).expect("write");
    assert_eq!(read(&path), vec![7, 9]);
    assert!(
        !path.with_extension("tmp").exists(),
        "the temp file was renamed into place"
    );
    write(&path, &[9]).expect("rewrite");
    assert_eq!(read(&path), vec![9]);
    clear(&path).expect("clear");
    assert!(!path.exists());
    clear(&path).expect("clearing a missing journal is fine");
    write(&path, &[]).expect("an empty write");
    assert!(!path.exists(), "an empty set leaves no file");
}

#[test]
fn restore_orphans_shows_only_orphans_then_clears() {
    let path = scratch("orphans");
    write(&path, &[3, 4, 5]).expect("write");
    let mut shown = Vec::new();
    let count = restore_orphans(&path, |h| h != 4, |h| shown.push(h));
    assert_eq!(count, 2);
    assert_eq!(
        shown,
        vec![3, 5],
        "a recycled or visible handle is left alone"
    );
    assert!(!path.exists(), "the journal is cleared after the restore");
    assert_eq!(
        restore_orphans(&path, |_| true, |_| panic!("nothing left")),
        0
    );
}
