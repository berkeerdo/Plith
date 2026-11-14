//! Offscreen rendering on WARP: the test hook that reads pixels back on the CPU.

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_MAP_OPTIONS_READ, ID2D1Bitmap1,
    ID2D1DeviceContext,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::{Error, Result};

use crate::device::Gpu;
use crate::paint::{PaintFrame, Painter};

fn make_bitmap(
    ctx: &ID2D1DeviceContext,
    width: u32,
    height: u32,
    dpi: f32,
    options: D2D1_BITMAP_OPTIONS,
) -> Result<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        bitmapOptions: options,
        colorContext: std::mem::ManuallyDrop::new(None),
    };
    // SAFETY: `props` is live for the call; no initial data is passed.
    unsafe { ctx.CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &raw const props) }
}

/// Paints `frame` into a `width_px` by `height_px` WARP bitmap at `scale` and returns tightly
/// packed premultiplied BGRA rows (`width_px * 4` bytes each).
///
/// # Errors
/// Returns the first failing Windows call, or `E_INVALIDARG` for a zero-sized target.
pub fn render_offscreen(
    width_px: u32,
    height_px: u32,
    scale: f64,
    frame: &PaintFrame,
) -> Result<Vec<u8>> {
    if width_px == 0 || height_px == 0 {
        return Err(Error::from(E_INVALIDARG));
    }
    let gpu = Gpu::new(true)?;
    let ctx = gpu.context();
    #[allow(clippy::cast_possible_truncation)]
    let dpi = (96.0 * scale) as f32;
    let target = make_bitmap(ctx, width_px, height_px, dpi, D2D1_BITMAP_OPTIONS_TARGET)?;
    // SAFETY: `target` is a live target bitmap created on this context.
    unsafe {
        ctx.SetTarget(&target);
        ctx.SetDpi(dpi, dpi);
    }
    Painter::new(ctx, gpu.d2d_factory())?.paint(ctx, frame)?;
    let staging = make_bitmap(
        ctx,
        width_px,
        height_px,
        dpi,
        D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    )?;
    let row = width_px as usize * 4;
    let mut out = Vec::with_capacity(row * height_px as usize);
    // SAFETY: the mapped region is `pitch * height` bytes and stays valid until Unmap; each
    // row copy reads `row <= pitch` bytes inside it.
    unsafe {
        staging.CopyFromBitmap(None, &target, None)?;
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;
        let pitch = mapped.pitch as usize;
        for y in 0..height_px as usize {
            out.extend_from_slice(std::slice::from_raw_parts(mapped.bits.add(y * pitch), row));
        }
        staging.Unmap()?;
        ctx.SetTarget(None::<&windows::Win32::Graphics::Direct2D::ID2D1Image>);
    }
    Ok(out)
}
