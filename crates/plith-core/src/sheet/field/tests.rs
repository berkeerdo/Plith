use super::*;

fn typed(s: &str) -> TextField {
    let mut f = TextField::default();
    for c in s.chars() {
        f.edit(&SheetKey::Char(c));
    }
    f
}

#[test]
fn field_basic_editing() {
    let mut f = typed("İst");
    assert_eq!(f.text, "İst");
    assert_eq!(f.caret, 3, "caret counts chars, not bytes");
    assert_eq!(f.edit(&SheetKey::Backspace), FieldOutcome::Edited);
    assert_eq!((f.text.as_str(), f.caret), ("İs", 2));
    assert_eq!(f.edit(&SheetKey::Home), FieldOutcome::Moved);
    assert_eq!(f.caret, 0);
    assert_eq!(f.edit(&SheetKey::Backspace), FieldOutcome::Ignored);
    f.edit(&SheetKey::Char('Ş'));
    assert_eq!((f.text.as_str(), f.caret), ("Şİs", 1));
    assert_eq!(f.edit(&SheetKey::Delete), FieldOutcome::Edited);
    assert_eq!(f.text, "Şs");
    f.edit(&SheetKey::End);
    assert_eq!(f.caret, 2);
    assert_eq!(f.edit(&SheetKey::Right), FieldOutcome::Ignored);
    assert_eq!(f.edit(&SheetKey::Left), FieldOutcome::Moved);
    assert_eq!(f.caret, 1);
    f.edit(&SheetKey::Space);
    assert_eq!(f.text, "Ş s");
}

#[test]
fn field_paste_cut_at_64() {
    let mut f = typed("ab");
    let long = "ü".repeat(100);
    assert_eq!(f.edit(&SheetKey::Paste(long)), FieldOutcome::Edited);
    assert_eq!(f.text.chars().count(), 64);
    assert_eq!(f.caret, 64);
    assert_eq!(f.edit(&SheetKey::Char('x')), FieldOutcome::Ignored);
    assert_eq!(
        f.edit(&SheetKey::Paste("more".into())),
        FieldOutcome::Ignored
    );
    let mut g = typed("a");
    g.edit(&SheetKey::Paste("b\nc\td".into()));
    assert_eq!(g.text, "abcd", "control characters are dropped");
}

#[test]
fn field_select_all_replaces_and_copies() {
    let mut f = typed("Izmir");
    assert_eq!(f.edit(&SheetKey::SelectAll), FieldOutcome::Moved);
    assert!(f.all);
    assert_eq!(f.edit(&SheetKey::Copy), FieldOutcome::Copy("Izmir".into()));
    f.edit(&SheetKey::Char('A'));
    assert_eq!((f.text.as_str(), f.caret, f.all), ("A", 1, false));
    f.edit(&SheetKey::SelectAll);
    f.edit(&SheetKey::Backspace);
    assert_eq!(f.text, "");
    assert_eq!(f.edit(&SheetKey::Copy), FieldOutcome::Ignored);
}

#[test]
fn field_escape_cancels() {
    let mut f = typed("abc");
    assert_eq!(f.edit(&SheetKey::Escape), FieldOutcome::Cancel);
    assert_eq!(f.text, "abc", "the field itself does not restore");
    assert_eq!(f.edit(&SheetKey::Enter), FieldOutcome::Commit("abc".into()));
    assert_eq!(f.edit(&SheetKey::Tab), FieldOutcome::Ignored);
}

#[test]
fn select_all_then_empty_insert_keeps_text() {
    let mut f = typed("Izmir");
    f.edit(&SheetKey::SelectAll);
    assert_eq!(f.edit(&SheetKey::Paste("\n".into())), FieldOutcome::Ignored);
    assert_eq!(
        f.edit(&SheetKey::Paste(String::new())),
        FieldOutcome::Ignored
    );
    assert_eq!(f.edit(&SheetKey::Char('\t')), FieldOutcome::Ignored);
    assert_eq!((f.text.as_str(), f.all), ("Izmir", true));
    assert_eq!(f.edit(&SheetKey::Char('a')), FieldOutcome::Edited);
    assert_eq!((f.text.as_str(), f.caret, f.all), ("a", 1, false));
    f.edit(&SheetKey::SelectAll);
    let full = "ü".repeat(70);
    f.edit(&SheetKey::Paste(full));
    assert_eq!(
        f.text.chars().count(),
        64,
        "a replaced selection frees the room"
    );
}
