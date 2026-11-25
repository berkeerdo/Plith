//! Decoding the installer art once, at the size that matches the window's scale.

use crate::art::{BACKGROUNDS, BUTTON};
use plith_core::draw::{ArtId, ArtPixels};
use plith_core::setup::{BG_ART, BUTTON_ART};
use std::sync::Arc;

/// From this many pixels per DIP on, the 2x files are used.
const TWO_X_FROM: f64 = 1.5;

/// The four backgrounds and the button, decoded from the 1x or 2x files by `scale` (pixels per
/// DIP). Each keeps its own pixel size: the renderer draws it into the DIP rectangle the
/// screen names. Needs COM on the calling thread.
///
/// # Errors
/// Names the first file that does not decode.
pub fn decode_all(scale: f64) -> Result<Vec<Arc<ArtPixels>>, String> {
    let pick =
        |pair: (&'static [u8], &'static [u8])| if scale >= TWO_X_FROM { pair.1 } else { pair.0 };
    let files = BG_ART
        .iter()
        .zip(BACKGROUNDS)
        .map(|(id, pair)| (*id, pick(pair)))
        .chain(std::iter::once((BUTTON_ART, pick(BUTTON))));
    files.map(|(id, bytes)| decode(id, bytes)).collect()
}

/// One file at its own size (a longest-side cap of `u32::MAX` never scales).
fn decode(id: ArtId, bytes: &[u8]) -> Result<Arc<ArtPixels>, String> {
    let art = plith_media::art::decode_image(bytes, u32::MAX)
        .ok_or_else(|| format!("cannot decode installer art {id:?}"))?;
    Ok(Arc::new(ArtPixels {
        id,
        width: art.width,
        height: art.height,
        bgra: art.bgra,
    }))
}
