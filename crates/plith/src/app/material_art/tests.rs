use super::*;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

fn com() {
    // SAFETY: initializes COM for this test thread; a repeat call returns S_FALSE and is fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

fn halka() -> ThemeId {
    ThemeId::parse("halka").unwrap()
}

fn size(p: &ArtPixels) -> (u32, u32) {
    (p.width, p.height)
}

#[test]
fn the_file_scale_follows_the_display_scale() {
    com();
    let pack = builtin_pack(&halka()).unwrap();
    let one = load("halka", pack, 1.0).unwrap();
    let two = load("halka", pack, 1.5).unwrap();
    assert_eq!(size(&one.fabric), (640, 420));
    assert_eq!(size(&two.fabric), (1280, 840));
    assert_eq!(one.fabric.bgra.len(), 640 * 420 * 4);
    assert_eq!(size(&two.disc).0, size(&one.disc).0 * 2);
}

#[test]
fn a_missing_file_names_the_pack_and_the_file() {
    com();
    let pack = BuiltinPack {
        manifest: "",
        files: &[],
    };
    let error = load("halka", &pack, 1.0).unwrap_err();
    assert_eq!(error, "theme art: halka floor-1x");
}

#[test]
fn a_file_that_is_not_an_image_names_the_pack_and_the_file() {
    com();
    let pack = BuiltinPack {
        manifest: "",
        files: &[("floor-1x", b"not a png")],
    };
    let error = load("halka", &pack, 1.0).unwrap_err();
    assert_eq!(error, "theme art: halka floor-1x");
}

#[test]
fn the_cache_decodes_once_per_scale_class() {
    com();
    let mut cache = ArtCache::default();
    let first = cache.get(&halka(), 1.0).unwrap();
    let again = cache.get(&halka(), 1.25).unwrap();
    assert!(Arc::ptr_eq(&first, &again), "same class, same Arc");
    let other = cache.get(&halka(), 2.0).unwrap();
    assert!(!Arc::ptr_eq(&first, &other));
}

#[test]
fn gece_has_no_pack_files_to_decode() {
    com();
    let mut cache = ArtCache::default();
    assert!(cache.get(&ThemeId::gece(), 1.0).is_err());
}
