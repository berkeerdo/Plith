//! Tests for artwork sizing, hashing, the reuse cache and the WIC decode.

use std::sync::Arc;

use super::{ArtCache, Artwork, MAX_SIDE, fit_longest, hash_bytes};

fn artwork(id: u64) -> Artwork {
    Artwork {
        width: 1,
        height: 1,
        bgra: Arc::from(vec![0u8; 4]),
        id,
    }
}

#[test]
fn scales_longest_side_to_256() {
    assert_eq!(fit_longest(512, 512, MAX_SIDE), (256, 256));
    assert_eq!(fit_longest(1000, 500, MAX_SIDE), (256, 128));
    assert_eq!(fit_longest(500, 1000, MAX_SIDE), (128, 256));
    assert_eq!(fit_longest(300, 200, MAX_SIDE), (256, 171));
}

#[test]
fn smaller_images_keep_their_size() {
    assert_eq!(fit_longest(2, 2, MAX_SIDE), (2, 2));
    assert_eq!(fit_longest(256, 100, MAX_SIDE), (256, 100));
    assert_eq!(fit_longest(0, 0, MAX_SIDE), (0, 0));
}

#[test]
fn thin_images_never_collapse_to_zero() {
    assert_eq!(fit_longest(10_000, 1, MAX_SIDE), (256, 1));
}

#[test]
fn hash_is_fnv1a() {
    assert_eq!(hash_bytes(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(hash_bytes(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_ne!(hash_bytes(b"ab"), hash_bytes(b"ba"));
}

#[test]
fn cache_decodes_once_per_thumbnail() {
    let mut cache = ArtCache::default();
    let bytes: Arc<[u8]> = Arc::from(vec![1u8, 2, 3]);
    let mut calls = 0;
    let first = cache.update(Some(&bytes), |_| {
        calls += 1;
        Some(artwork(1))
    });
    let again: Arc<[u8]> = Arc::from(vec![1u8, 2, 3]);
    let second = cache.update(Some(&again), |_| {
        calls += 1;
        Some(artwork(2))
    });
    assert_eq!(calls, 1);
    assert!(Arc::ptr_eq(&first.expect("art"), &second.expect("art")));
}

#[test]
fn cache_redecodes_when_bytes_change_and_clears_without_thumbnail() {
    let mut cache = ArtCache::default();
    let one: Arc<[u8]> = Arc::from(vec![1u8]);
    let two: Arc<[u8]> = Arc::from(vec![2u8]);
    cache.update(Some(&one), |_| Some(artwork(1)));
    let next = cache.update(Some(&two), |_| Some(artwork(2)));
    assert_eq!(next.expect("art").id, 2);
    assert!(cache.update(None, |_| Some(artwork(3))).is_none());
    assert!(cache.current().is_none());
}

#[test]
fn cache_remembers_a_failed_decode() {
    let mut cache = ArtCache::default();
    let bytes: Arc<[u8]> = Arc::from(vec![9u8]);
    let mut calls = 0;
    for _ in 0..2 {
        let got = cache.update(Some(&bytes), |_| {
            calls += 1;
            None
        });
        assert!(got.is_none());
    }
    assert_eq!(calls, 1);
}

#[cfg(windows)]
mod wic {
    use crate::art::wic::decode;
    use crate::com::ComGuard;

    /// A 2 x 2 RGBA PNG, row by row: opaque red, red at alpha 128 (straight), opaque green,
    /// fully transparent black.
    const PNG_2X2: [u8; 81] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x06, 0x00, 0x00, 0x00, 0x72,
        0xb6, 0x0d, 0x24, 0x00, 0x00, 0x00, 0x18, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x0d, 0xc5,
        0xb1, 0x01, 0x00, 0x00, 0x08, 0xc3, 0x20, 0x4e, 0xf7, 0xf3, 0x54, 0x16, 0x44, 0x71, 0xe4,
        0x67, 0x3c, 0xda, 0x05, 0x7c, 0x80, 0x09, 0x1e, 0x4a, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
        0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    /// Runs `f` on its own thread with COM initialized, the way the media worker does.
    fn with_com<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::spawn(move || {
            let _com = ComGuard::init().expect("com");
            f()
        })
        .join()
        .expect("thread")
    }

    #[test]
    fn decodes_png_to_premultiplied_bgra() {
        let art = with_com(|| decode(&PNG_2X2)).expect("decoded");
        assert_eq!((art.width, art.height), (2, 2));
        assert_eq!(art.id, crate::art::hash_bytes(&PNG_2X2));
        // B, G, R, A per pixel: opaque red, half-transparent red premultiplied, opaque green,
        // transparent.
        let expected: [[u8; 4]; 4] = [
            [0, 0, 255, 255],
            [0, 0, 128, 128],
            [0, 255, 0, 255],
            [0, 0, 0, 0],
        ];
        for (i, want) in expected.iter().enumerate() {
            let got = &art.bgra[i * 4..i * 4 + 4];
            for (g, w) in got.iter().zip(want) {
                assert!(g.abs_diff(*w) <= 1, "pixel {i}: got {got:?}, want {want:?}");
            }
        }
    }

    #[test]
    fn garbage_bytes_give_none() {
        assert!(with_com(|| decode(b"this is not an image")).is_none());
        assert!(with_com(|| decode(&[])).is_none());
    }
}
