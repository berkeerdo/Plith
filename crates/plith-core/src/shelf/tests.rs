use super::*;

fn file(name: &str) -> ShelfItem {
    ShelfItem {
        path: PathBuf::from(format!("C:\\Users\\a\\{name}")),
        dir: false,
    }
}

fn names(shelf: &Shelf) -> Vec<String> {
    shelf
        .items()
        .iter()
        .map(|i| i.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_batch_lands_in_front_with_its_first_item_first() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("old")]);
    let added = shelf.add(&[file("a"), file("b")]);
    assert_eq!(names(&shelf), ["a", "b", "old"]);
    assert_eq!(added.new, 2);
}

#[test]
fn a_re_drop_moves_to_the_front_without_a_copy() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b"), file("c")]);
    let added = shelf.add(&[file("c")]);
    assert_eq!(names(&shelf), ["c", "a", "b"]);
    assert_eq!((added.new, added.moved), (0, 1));
}

#[test]
fn paths_compare_without_case_or_a_trailing_separator() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("Photo.PNG")]);
    shelf.add(&[file("photo.png")]);
    assert_eq!(shelf.len(), 1);
    let dir = |p: &str| ShelfItem {
        path: PathBuf::from(p),
        dir: true,
    };
    shelf.add(&[dir("C:\\x\\")]);
    shelf.add(&[dir("C:\\X")]);
    assert_eq!(shelf.len(), 2);
}

#[test]
fn a_repeat_inside_one_batch_counts_once() {
    let mut shelf = Shelf::default();
    let added = shelf.add(&[file("a"), file("a")]);
    assert_eq!((shelf.len(), added.new), (1, 1));
}

#[test]
fn the_ninth_drop_pushes_the_oldest_out() {
    let mut shelf = Shelf::default();
    for i in 0..8 {
        shelf.add(&[file(&format!("f{i}"))]);
    }
    assert_eq!(shelf.len(), CAPACITY);
    let added = shelf.add(&[file("new")]);
    assert_eq!(added.pushed_out, 1);
    assert_eq!(shelf.len(), CAPACITY);
    assert_eq!(shelf.items()[0].path, file("new").path);
    assert!(!names(&shelf).contains(&"f0".to_string()));
}

#[test]
fn a_relative_path_is_refused() {
    let mut shelf = Shelf::default();
    let relative = ShelfItem {
        path: PathBuf::from("notes.txt"),
        dir: false,
    };
    assert_eq!(shelf.add(&[relative]), Added::default());
    assert!(shelf.is_empty());
}

#[test]
fn remove_and_undo_restore_the_old_place() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b"), file("c")]);
    assert!(shelf.remove(1));
    assert_eq!(names(&shelf), ["a", "c"]);
    assert!(shelf.can_undo());
    assert!(shelf.undo());
    assert_eq!(names(&shelf), ["a", "b", "c"]);
    assert!(!shelf.can_undo());
    assert!(!shelf.undo());
}

#[test]
fn remove_past_the_end_does_nothing() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a")]);
    assert!(!shelf.remove(3));
    assert!(!shelf.can_undo());
}

#[test]
fn undo_keeps_only_the_last_removal() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b"), file("c")]);
    shelf.remove(0);
    shelf.remove(0);
    assert!(shelf.undo());
    assert_eq!(names(&shelf), ["b", "c"]);
}

#[test]
fn undo_is_refused_when_the_path_came_back_by_a_drop() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a")]);
    shelf.remove(0);
    shelf.add(&[file("a")]);
    assert!(!shelf.undo());
    assert_eq!(shelf.len(), 1);
}

#[test]
fn undo_on_a_full_shelf_stays_within_the_capacity() {
    let mut shelf = Shelf::default();
    for i in 0..8 {
        shelf.add(&[file(&format!("f{i}"))]);
    }
    shelf.remove(7);
    shelf.add(&[file("fresh")]);
    assert!(shelf.undo());
    assert_eq!(shelf.len(), CAPACITY);
}

#[test]
fn clear_empties_the_shelf_and_one_undo_brings_everything_back_in_order() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b"), file("c")]);
    assert_eq!(shelf.clear(), 3);
    assert!(shelf.is_empty());
    assert!(shelf.can_undo());
    assert!(shelf.undo());
    assert_eq!(names(&shelf), ["a", "b", "c"]);
    assert!(!shelf.can_undo());
}

#[test]
fn clearing_an_empty_shelf_offers_no_undo() {
    let mut shelf = Shelf::default();
    assert_eq!(shelf.clear(), 0);
    assert!(!shelf.can_undo());
}

#[test]
fn clear_replaces_an_earlier_removal_and_undo_skips_paths_that_came_back() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b"), file("c")]);
    shelf.remove(0);
    shelf.clear();
    shelf.add(&[file("b")]);
    assert!(shelf.undo());
    assert_eq!(names(&shelf), ["b", "c"]);
}

#[test]
fn the_text_form_is_one_path_per_line_newest_first() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b")]);
    assert_eq!(shelf.serialize(), "C:\\Users\\a\\a\nC:\\Users\\a\\b\n");
}

#[test]
fn parse_round_trips_the_text_form() {
    let mut shelf = Shelf::default();
    shelf.add(&[file("a"), file("b")]);
    let back = Shelf::parse(&shelf.serialize(), |_| Probe::File);
    assert_eq!(back.items(), shelf.items());
}

#[test]
fn parse_keeps_a_path_it_could_not_probe() {
    let text = "C:\\x\\a.txt\n\\\\nas\\share\\b.txt\n";
    let shelf = Shelf::parse(text, |p| {
        if p.to_string_lossy().starts_with("\\\\") {
            Probe::Unknown
        } else {
            Probe::File
        }
    });
    assert_eq!(names(&shelf), ["a.txt", "b.txt"]);
    assert!(shelf.serialize().contains("\\\\nas\\share\\b.txt"));
}

#[test]
fn newest_cuts_a_big_drop_to_eight_distinct_rooted_paths() {
    let mut paths: Vec<PathBuf> = (0..1000)
        .map(|i| PathBuf::from(format!("C:\\x\\f{i}")))
        .collect();
    paths.insert(0, PathBuf::from("relative"));
    paths.insert(2, PathBuf::from("c:\\X\\F0"));
    let cut = newest(&paths);
    assert_eq!(cut.len(), CAPACITY);
    assert_eq!(cut[0], PathBuf::from("C:\\x\\f0"));
    assert_eq!(cut[1], PathBuf::from("C:\\x\\f1"));
}

#[test]
fn append_older_goes_behind_without_repeats_within_the_capacity() {
    let mut now = Shelf::default();
    now.add(&[file("new")]);
    let mut old = Shelf::default();
    for i in 0..8 {
        old.add(&[file(&format!("o{i}"))]);
    }
    old.add(&[file("new")]);
    now.append_older(&old);
    assert_eq!(now.len(), CAPACITY);
    assert_eq!(names(&now)[0], "new");
    assert_eq!(names(&now).iter().filter(|n| *n == "new").count(), 1);
}

#[test]
fn parse_drops_what_is_gone_relative_blank_or_repeated() {
    let text = "\u{feff}C:\\x\\gone.txt\r\n\r\nrelative.txt\r\nC:\\x\\a.txt\r\nc:\\X\\A.txt\r\nC:\\x\\dir\r\n";
    let shelf = Shelf::parse(text, |p| {
        let s = p.to_string_lossy().to_lowercase();
        if s.ends_with("gone.txt") {
            Probe::Gone
        } else if s.ends_with("dir") {
            Probe::Dir
        } else {
            Probe::File
        }
    });
    assert_eq!(names(&shelf), ["a.txt", "dir"]);
    assert!(!shelf.items()[0].dir && shelf.items()[1].dir);
}

#[test]
fn parse_keeps_only_the_first_eight_lines() {
    let text = (0..12)
        .map(|i| format!("C:\\x\\f{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let shelf = Shelf::parse(&text, |_| Probe::File);
    assert_eq!(shelf.len(), CAPACITY);
    assert_eq!(names(&shelf)[0], "f0");
}
