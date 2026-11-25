//! PNG helpers shared by the render examples: write BGRA rows with WIC, decode a PNG file to
//! premultiplied BGRA. COM must be initialized on the calling thread.

use std::path::Path;
use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    GUID_WICPixelFormat32bppPBGRA, IWICBitmapFrameEncode, IWICBitmapSource, IWICImagingFactory,
    IWICPalette, WICBitmapDitherTypeNone, WICBitmapEncoderNoCache, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{HSTRING, Interface, Result};

/// Writes `w` by `h` BGRA rows as a PNG with WIC.
pub fn write_png(path: &Path, rows: &[u8], w: u32, h: u32) -> Result<()> {
    // SAFETY: COM is initialized on this thread before the factory is created; every pointer
    // passed points to a live local, and `rows` holds `h` rows of `w * 4` bytes.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame: Option<IWICBitmapFrameEncode> = None;
        encoder.CreateNewFrame(&raw mut frame, std::ptr::null_mut())?;
        let frame = frame.ok_or_else(windows::core::Error::empty)?;
        frame.Initialize(None)?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&raw mut format)?;
        frame.WritePixels(h, w * 4, rows)?;
        frame.Commit()?;
        encoder.Commit()?;
    }
    Ok(())
}

/// Decodes a PNG to premultiplied BGRA as `(width, height, rows)`. With `side` set it is first
/// scaled to `side` by `side` with Fant; with `None` it keeps its own size.
pub fn decode_png(path: &Path, side: Option<u32>) -> Result<(u32, u32, Vec<u8>)> {
    // SAFETY: COM is initialized on this thread; every out pointer is a live local.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = factory.CreateDecoderFromFilename(
            &HSTRING::from(path.as_os_str()),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let mut source: IWICBitmapSource = decoder.GetFrame(0)?.cast()?;
        if let Some(side) = side {
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(&source, side, side, WICBitmapInterpolationModeFant)?;
            source = scaler.cast()?;
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
        let (mut w, mut h) = (0u32, 0u32);
        converter.GetSize(&raw mut w, &raw mut h)?;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        converter.CopyPixels(std::ptr::null(), w * 4, &mut pixels)?;
        Ok((w, h, pixels))
    }
}
