use super::*;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

fn com() {
    // SAFETY: initializes COM for this test thread; a repeat call returns S_FALSE and is fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

#[test]
fn icon_art_decodes_the_tile() {
    com();
    let art = icon_art(TILE_PNG, 2.0).expect("the embedded tile decodes");
    assert_eq!(art.id, ICON_ART);
    assert_eq!((art.width, art.height), (192, 192));
    assert_eq!(art.bgra.len(), 192 * 192 * 4);
}

#[test]
fn icon_art_is_decoded_at_the_window_scale() {
    // Drawn 1:1, so Direct2D never minifies the fabric weave (review: moire at 1x and 1.5x).
    com();
    let side = |scale: f64| icon_art(TILE_PNG, scale).map(|a| (a.width, a.height));
    assert_eq!(side(1.0), Some((96, 96)));
    assert_eq!(side(1.5), Some((144, 144)));
    assert_eq!(side(3.0), Some((256, 256)), "never above the tile");
}

#[test]
fn icon_art_is_none_for_garbage() {
    com();
    assert!(icon_art(b"not a png", 1.0).is_none());
}
