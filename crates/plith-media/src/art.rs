//! Artwork decoding: encoded PNG or JPEG bytes from SMTC become premultiplied BGRA pixels.
//!
//! The pure part (size arithmetic, hashing, the reuse cache) is unit tested everywhere. The WIC
//! part runs on the media worker thread, which already owns a COM apartment, so the UI and
//! render threads never pay for a decode.

use std::sync::Arc;

use crate::snapshot::TrackFacts;

/// Longest side of decoded artwork, in pixels. Larger images are scaled down, never up.
pub const MAX_SIDE: u32 = 256;

/// Decoded artwork: premultiplied BGRA, 4 bytes per pixel, rows tightly packed.
#[derive(Debug, Clone)]
pub struct Artwork {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes of premultiplied BGRA.
    pub bgra: Arc<[u8]>,
    /// Stable hash of the encoded source bytes; equal ids mean the same picture.
    pub id: u64,
}

impl PartialEq for Artwork {
    /// Two artworks are equal when they come from the same encoded bytes at the same size.
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.width == other.width && self.height == other.height
    }
}

impl Eq for Artwork {}

/// Scales `(w, h)` down so the longest side is at most `max`, keeping the aspect ratio.
/// Smaller images keep their size. Neither side ever drops below 1; zero stays zero.
#[must_use]
pub fn fit_longest(w: u32, h: u32, max: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max || longest == 0 {
        return (w, h);
    }
    let scale = |side: u32| -> u32 {
        let scaled =
            (u64::from(side) * u64::from(max) + u64::from(longest) / 2) / u64::from(longest);
        u32::try_from(scaled).unwrap_or(max).max(1)
    };
    (scale(w), scale(h))
}

/// 64-bit FNV-1a of `bytes`.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Remembers the last decoded artwork so an unchanged thumbnail is decoded only once.
#[derive(Debug, Default)]
pub(crate) struct ArtCache {
    source: Option<Arc<[u8]>>,
    artwork: Option<Arc<Artwork>>,
}

impl ArtCache {
    /// The artwork for `thumbnail`. The previous `Arc` is reused when the bytes are the same
    /// allocation or equal; otherwise `decode` runs once. No thumbnail clears the cache. A
    /// failed decode is remembered too, so garbage bytes are not retried on every read.
    pub(crate) fn update(
        &mut self,
        thumbnail: Option<&Arc<[u8]>>,
        decode: impl FnOnce(&[u8]) -> Option<Artwork>,
    ) -> Option<Arc<Artwork>> {
        let Some(bytes) = thumbnail else {
            self.source = None;
            self.artwork = None;
            return None;
        };
        let same = self
            .source
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, bytes) || old[..] == bytes[..]);
        if !same {
            self.artwork = decode(bytes).map(Arc::new);
            self.source = Some(Arc::clone(bytes));
        }
        self.artwork.clone()
    }
}

impl ArtCache {
    /// The artwork of the last refresh.
    pub(crate) fn current(&self) -> Option<Arc<Artwork>> {
        self.artwork.clone()
    }

    /// Refreshes from the current track, decoding with WIC when its thumbnail is new.
    #[cfg(windows)]
    pub(crate) fn refresh(&mut self, track: Option<&TrackFacts>) {
        self.update(track.and_then(|t| t.thumbnail.as_ref()), wic::decode);
    }
}

#[cfg(windows)]
mod wic {
    use std::sync::Arc;

    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource,
        IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
        WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};

    use super::{Artwork, MAX_SIDE, fit_longest, hash_bytes};

    /// Decodes encoded image bytes into premultiplied BGRA, longest side at most [`MAX_SIDE`].
    /// Needs COM initialized on the calling thread. `None` when the bytes are not an image.
    pub(crate) fn decode(encoded: &[u8]) -> Option<Artwork> {
        let (width, height, pixels) = decode_pixels(encoded).ok()?;
        Some(Artwork {
            width,
            height,
            bgra: Arc::from(pixels),
            id: hash_bytes(encoded),
        })
    }

    fn decode_pixels(encoded: &[u8]) -> windows::core::Result<(u32, u32, Vec<u8>)> {
        // SAFETY: COM is initialized on this thread by the caller, and every out pointer handed
        // to WIC is a local that outlives its call. `InitializeFromMemory` references `encoded`
        // without copying it; the stream, the decoder, the frame and everything built on them
        // are locals of this block, so they are all dropped before `encoded` (borrowed for the
        // whole call) goes out of scope.
        unsafe {
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let stream = factory.CreateStream()?;
            stream.InitializeFromMemory(encoded)?;
            let decoder = factory.CreateDecoderFromStream(
                &stream,
                std::ptr::null(),
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let (mut w, mut h) = (0u32, 0u32);
            frame.GetSize(&raw mut w, &raw mut h)?;
            let (out_w, out_h) = fit_longest(w, h, MAX_SIDE);
            if out_w == 0 || out_h == 0 {
                // E_INVALIDARG: an image with an empty side has nothing to decode.
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(-2_147_024_809),
                    "empty image",
                ));
            }
            let mut source: IWICBitmapSource = windows::core::Interface::cast(&frame)?;
            if (out_w, out_h) != (w, h) {
                let scaler = factory.CreateBitmapScaler()?;
                scaler.Initialize(&source, out_w, out_h, WICBitmapInterpolationModeFant)?;
                source = windows::core::Interface::cast(&scaler)?;
            }
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(
                &source,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            let stride = out_w * 4;
            let mut pixels = vec![0u8; stride as usize * out_h as usize];
            converter.CopyPixels(std::ptr::null(), stride, &mut pixels)?;
            Ok((out_w, out_h, pixels))
        }
    }
}

#[cfg(test)]
mod tests;
