//! The move off the retired themes (Sodyum, York, Kağıt): each becomes Gece with the floor and
//! accent that made it look the way it did.

use super::Settings;
use crate::theme::colour::Rgb;
use crate::theme::registry::{AccentPick, ThemeId};

/// Sets Gece's floor and accent unless the user already has their own.
fn gece_look(s: &mut Settings, floor: Option<[u8; 3]>, accent: [u8; 3]) {
    let gece = ThemeId::gece();
    if let Some(floor) = floor {
        s.floors.entry(gece.clone()).or_insert(Rgb(floor));
    }
    s.accents
        .entry(gece)
        .or_insert(AccentPick::Colour(Rgb(accent)));
}

/// The Kağıt look on Gece: its paper floor and dark green accent. Keeps a floor or accent the
/// settings already hold.
pub(super) fn kagit(s: &mut Settings) {
    gece_look(s, Some([0xF3, 0xF2, 0xED]), [0x3F, 0x6E, 0x00]);
}

/// Moves a retired theme named by `raw_theme` (the last good `theme=` value) onto Gece. A floor
/// or accent the file already has for Gece is never overwritten. Returns whether anything
/// changed.
pub fn retired(s: &mut Settings, raw_theme: &str) -> bool {
    match raw_theme {
        "sodyum" => gece_look(s, None, [0xFF, 0xB2, 0x24]),
        "york" => gece_look(s, Some([0x1C, 0x24, 0x20]), [0xFF, 0xE9, 0xA8]),
        "kagit" => kagit(s),
        _ => return false,
    }
    s.theme = ThemeId::gece();
    true
}
