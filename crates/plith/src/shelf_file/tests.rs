use super::*;
use plith_core::shelf::ShelfItem;

fn folder(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("plith-shelf-{name}-{}", std::process::id()))
}

fn item(path: PathBuf, dir: bool) -> ShelfItem {
    ShelfItem { path, dir }
}

#[test]
fn a_missing_file_is_an_empty_shelf() {
    assert!(load_at(&folder("missing").join("shelf.txt")).is_empty());
}

#[test]
fn a_saved_shelf_reads_back_in_order() {
    let dir = folder("round");
    fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    let mut shelf = Shelf::default();
    shelf.add(&[item(a.clone(), false), item(b.clone(), false)]);
    let at = dir.join("shelf.txt");
    save_at(&at, &shelf).unwrap();
    let back = load_at(&at);
    assert_eq!(back.items(), shelf.items());
    assert_eq!(back.items()[0].path, a);
}

#[test]
fn load_drops_what_is_gone_and_knows_folders() {
    let dir = folder("gone");
    fs::create_dir_all(dir.join("sub")).unwrap();
    let kept = dir.join("kept.txt");
    fs::write(&kept, "x").unwrap();
    let text = format!(
        "{}\n{}\n{}\n",
        dir.join("vanished.txt").display(),
        dir.join("sub").display(),
        kept.display()
    );
    fs::write(dir.join("shelf.txt"), text).unwrap();
    let shelf = load_at(&dir.join("shelf.txt"));
    assert_eq!(shelf.len(), 2);
    assert!(shelf.items()[0].dir);
    assert!(!shelf.items()[1].dir);
}

#[test]
fn saving_replaces_the_old_file_through_a_temp_file() {
    let dir = folder("atomic");
    let at = dir.join("shelf.txt");
    let mut shelf = Shelf::default();
    shelf.add(&[item(dir.join("one.txt"), false)]);
    save_at(&at, &shelf).unwrap();
    shelf.add(&[item(dir.join("two.txt"), false)]);
    save_at(&at, &shelf).unwrap();
    assert_eq!(fs::read_to_string(&at).unwrap(), shelf.serialize());
    let mut temp = at.as_os_str().to_owned();
    temp.push(".tmp");
    assert!(!PathBuf::from(temp).exists(), "the temp file is gone");
}
