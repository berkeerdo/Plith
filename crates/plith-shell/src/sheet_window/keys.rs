//! Maps Win32 key messages to [`SheetKey`]. Pure: no window, no clipboard.

use plith_core::sheet::SheetKey;

/// What a `WM_KEYDOWN` means to the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KeyMap {
    /// A key the sheet understands as is.
    Key(SheetKey),
    /// Ctrl+V: the window reads the clipboard and sends `SheetKey::Paste` with its text.
    Paste,
}

const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_RETURN: u16 = 0x0D;
const VK_ESCAPE: u16 = 0x1B;
const VK_SPACE: u16 = 0x20;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_DELETE: u16 = 0x2E;

/// Maps a virtual key and the modifier state of a `WM_KEYDOWN`. Plain letters return `None`:
/// their text arrives through `WM_CHAR`. Ctrl+Alt is `AltGr` and also produces text.
pub(super) fn map_keydown(vk: u16, ctrl: bool, shift: bool, alt: bool) -> Option<KeyMap> {
    if ctrl && !alt {
        return match u8::try_from(vk).ok()? {
            b'A' => Some(KeyMap::Key(SheetKey::SelectAll)),
            b'C' => Some(KeyMap::Key(SheetKey::Copy)),
            b'V' => Some(KeyMap::Paste),
            _ => None,
        };
    }
    let key = match vk {
        VK_TAB if shift => SheetKey::BackTab,
        VK_TAB => SheetKey::Tab,
        VK_LEFT => SheetKey::Left,
        VK_RIGHT => SheetKey::Right,
        VK_UP => SheetKey::Up,
        VK_DOWN => SheetKey::Down,
        VK_HOME => SheetKey::Home,
        VK_END => SheetKey::End,
        VK_BACK => SheetKey::Backspace,
        VK_DELETE => SheetKey::Delete,
        VK_RETURN => SheetKey::Enter,
        VK_SPACE => SheetKey::Space,
        VK_ESCAPE => SheetKey::Escape,
        _ => return None,
    };
    Some(KeyMap::Key(key))
}

/// Maps one `WM_CHAR` character. Control characters and Space are ignored here: their keys come
/// from `WM_KEYDOWN`.
pub(super) fn map_char(ch: char) -> Option<SheetKey> {
    if ch.is_control() || ch == ' ' {
        None
    } else {
        Some(SheetKey::Char(ch))
    }
}

/// Joins the UTF-16 units of `WM_CHAR` into characters.
#[derive(Debug, Default)]
pub(super) struct CharAssembler {
    high: Option<u16>,
}

impl CharAssembler {
    /// Feeds one unit; returns a character once it is complete. Unpaired surrogates are dropped.
    pub(super) fn push(&mut self, unit: u16) -> Option<char> {
        let pending = self.high.take();
        match unit {
            0xD800..=0xDBFF => {
                self.high = Some(unit);
                None
            }
            0xDC00..=0xDFFF => {
                let high = pending?;
                char::decode_utf16([high, unit]).next()?.ok()
            }
            _ => char::from_u32(u32::from(unit)),
        }
    }
}

#[cfg(test)]
mod tests;
