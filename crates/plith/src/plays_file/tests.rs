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
