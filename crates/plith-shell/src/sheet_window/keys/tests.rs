use super::*;

const VK_TAB: u16 = 0x09;
const VK_ESCAPE: u16 = 0x1B;
const VK_SPACE: u16 = 0x20;
const VK_RETURN: u16 = 0x0D;
const VK_LEFT: u16 = 0x25;
const VK_DELETE: u16 = 0x2E;

#[allow(clippy::unnecessary_wraps)] // reads as the expected value of `map_keydown`
fn key(k: SheetKey) -> Option<KeyMap> {
    Some(KeyMap::Key(k))
}

#[test]
fn keys_map() {
    assert_eq!(map_keydown(VK_TAB, false, false, false), key(SheetKey::Tab));
    assert_eq!(
        map_keydown(VK_TAB, false, true, false),
        key(SheetKey::BackTab)
    );
    assert_eq!(
        map_keydown(VK_ESCAPE, false, false, false),
        key(SheetKey::Escape)
    );
    assert_eq!(
        map_keydown(VK_SPACE, false, false, false),
        key(SheetKey::Space)
    );
    assert_eq!(
        map_keydown(VK_RETURN, false, false, false),
        key(SheetKey::Enter)
    );
    assert_eq!(
        map_keydown(VK_LEFT, false, false, false),
        key(SheetKey::Left)
    );
    assert_eq!(
        map_keydown(VK_DELETE, false, false, false),
        key(SheetKey::Delete)
    );
    assert_eq!(
        map_keydown(u16::from(b'A'), true, false, false),
        key(SheetKey::SelectAll)
    );
    assert_eq!(
        map_keydown(u16::from(b'C'), true, false, false),
        key(SheetKey::Copy)
    );
    assert_eq!(
        map_keydown(u16::from(b'V'), true, false, false),
        Some(KeyMap::Paste)
    );
    // A plain letter arrives through WM_CHAR, not WM_KEYDOWN; Ctrl+Alt is AltGr text.
    assert_eq!(map_keydown(u16::from(b'A'), false, false, false), None);
    assert_eq!(map_keydown(u16::from(b'A'), true, false, true), None);
}

#[test]
fn chars_map() {
    assert_eq!(map_char('ş'), Some(SheetKey::Char('ş')));
    assert_eq!(map_char('a'), Some(SheetKey::Char('a')));
    for control in ['\u{1}', '\u{8}', '\t', '\r', '\u{1b}', '\u{7f}', ' '] {
        assert_eq!(map_char(control), None, "{control:?}");
    }
}

#[test]
fn surrogate_pairs_join() {
    let mut a = CharAssembler::default();
    assert_eq!(a.push(0xD83D), None);
    assert_eq!(a.push(0xDE00), Some('\u{1F600}'));
    assert_eq!(a.push(u16::from(b'x')), Some('x'));
    // A lone high surrogate followed by a plain unit drops the lone one.
    assert_eq!(a.push(0xD83D), None);
    assert_eq!(a.push(u16::from(b'y')), Some('y'));
    // A lone low surrogate is dropped.
    assert_eq!(a.push(0xDE00), None);
}
