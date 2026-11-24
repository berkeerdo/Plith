//! The pure parts of the icon's messages: which event a callback carries, where the menu goes
//! and the tooltip buffer.

/// `NIN_SELECT`: the icon was clicked with the left button (version 4).
const NIN_SELECT: u32 = 0x400;
/// `NIN_KEYSELECT`: the icon was chosen with the keyboard.
const NIN_KEYSELECT: u32 = 0x401;
/// `WM_CONTEXTMENU`: the right button, or the menu key, on the icon.
const WM_CONTEXTMENU: u32 = 0x7B;

/// Length of the tooltip buffer in UTF-16 units, the terminator included.
pub(super) const TIP_LEN: usize = 128;

/// What a callback asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Click {
    /// Open the settings window.
    Open,
    /// Show the menu.
    Menu,
    /// Nothing: moves, presses and the other notifications.
    Ignore,
}

/// The event code of a version 4 callback: the low word of `lparam`.
pub(super) fn event_of(lparam: isize) -> u32 {
    u32::try_from(lparam & 0xFFFF).unwrap_or(0)
}

/// The signed 16-bit word `shift` bits up in `value`.
fn signed_word(value: usize, shift: u32) -> i32 {
    let word = u16::try_from((value >> shift) & 0xFFFF).unwrap_or(0);
    i32::from(word.cast_signed())
}

/// What an event code asks for. A left click is `NIN_SELECT` alone: the raw button messages come
/// as well and would open the window twice.
pub(super) fn click_of(event: u32) -> Click {
    match event {
        NIN_SELECT | NIN_KEYSELECT => Click::Open,
        WM_CONTEXTMENU => Click::Menu,
        _ => Click::Ignore,
    }
}

/// The screen point the icon gave for the menu: the signed low and high words of `wparam`.
pub(super) fn anchor_of(wparam: usize) -> (i32, i32) {
    (signed_word(wparam, 0), signed_word(wparam, 16))
}

/// `text` as a NUL-terminated tooltip buffer, cut to fit.
pub(super) fn fill_tip(text: &str) -> [u16; TIP_LEN] {
    let mut tip = [0u16; TIP_LEN];
    for (slot, unit) in tip.iter_mut().take(TIP_LEN - 1).zip(text.encode_utf16()) {
        *slot = unit;
    }
    tip
}

#[cfg(test)]
mod tests;
