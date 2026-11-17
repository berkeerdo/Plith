use super::*;

fn path(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!("plith-plays-{name}-{}", std::process::id()))
        .join("plays.txt")
}

#[test]
fn a_missing_file_is_an_empty_log() {
    let log = load_at(&path("missing"));
    assert_eq!(log.serialize(), "");
}

#[test]
fn a_saved_log_reads_back() {
    let mut plays = PlayLog::default();
    plays.observe((2026, 11, 17), 600, 0, Some(("Holocene", "Bon Iver")), true);
    plays.observe((2026, 11, 17), 642, 0, None, false);
    let at = path("round");
    save_at(&at, &plays).unwrap();
    assert_eq!(load_at(&at).serialize(), plays.serialize());
    assert_eq!(load_at(&at).in_window(0, 1440).len(), 1);
}

#[test]
fn saving_replaces_the_old_file_through_a_temp_file() {
    let at = path("atomic");
    let mut old = PlayLog::default();
    old.observe((2026, 11, 17), 600, 0, Some(("Old", "Band")), true);
    old.observe((2026, 11, 17), 650, 0, None, false);
    save_at(&at, &old).unwrap();
    let mut newer = PlayLog::default();
    newer.observe((2026, 11, 17), 700, 0, Some(("New", "Band")), true);
    newer.observe((2026, 11, 17), 760, 0, None, false);
    save_at(&at, &newer).unwrap();
    assert_eq!(load_at(&at).serialize(), newer.serialize());
    let mut temp = at.as_os_str().to_owned();
    temp.push(".tmp");
    assert!(!PathBuf::from(temp).exists(), "the temp file is gone");
}
