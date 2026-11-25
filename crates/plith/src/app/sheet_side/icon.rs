//! The app icon tile for the settings window's About section, decoded once when the window opens.

use plith_core::draw::ArtPixels;
use plith_core::sheet::{ICON_ART, ICON_DIP};
use std::sync::Arc;

/// The 256 px icon tile, written by `scripts/build-icon.ps1` from the icon master.
pub(super) const TILE_PNG: &[u8] = include_bytes!("../../../assets/plith-tile-256.png");

/// Side of the embedded tile in pixels: the largest the icon is ever decoded at.
const TILE_SIDE: u32 = 256;

/// Decodes `encoded` into the About icon's artwork at its drawn size for `scale` (pixels per
/// DIP), so Direct2D draws it 1:1 and never minifies the fabric weave. Needs COM on the calling
/// thread (the UI thread has it through `OleInitialize`). `None` when the bytes are not an image.
pub(super) fn icon_art(encoded: &[u8], scale: f64) -> Option<Arc<ArtPixels>> {
    // A few hundred pixels at most; a NaN or negative scale falls back to the tile size.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let side = (ICON_DIP * scale).round().clamp(1.0, f64::from(TILE_SIDE)) as u32;
    let side = if scale.is_finite() && scale > 0.0 {
        side
    } else {
        TILE_SIDE
    };
    let art = plith_media::art::decode_image(encoded, side)?;
    Some(Arc::new(ArtPixels {
        id: ICON_ART,
        width: art.width,
        height: art.height,
        bgra: art.bgra,
    }))
}

#[cfg(test)]
mod tests;
