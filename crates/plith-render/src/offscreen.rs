//! Offscreen rendering on WARP: the test hook that reads pixels back on the CPU.

use plith_core::material::art::ThemePaint;
use plith_core::theme::Palette;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_MAP_OPTIONS_READ, ID2D1Bitmap1,
    ID2D1DeviceContext, ID2D1Image,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::{Error, Result};

use crate::device::Gpu;
use crate::paint::{PaintFrame, Painter};
use crate::text::TextSystem;

pub(crate) fn make_bitmap(
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

/// Copies `target` into the CPU-readable `staging` bitmap (both `width` by `height` pixels) and
/// returns tightly packed premultiplied BGRA rows.
pub(crate) fn read_back(
    staging: &ID2D1Bitmap1,
    target: &ID2D1Bitmap1,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let row = width as usize * 4;
    let mut out = Vec::with_capacity(row * height as usize);
    // SAFETY: the mapped region is `pitch * height` bytes and stays valid until Unmap; each
    // row copy reads `row <= pitch` bytes inside it.
    unsafe {
        staging.CopyFromBitmap(None, target, None)?;
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;
        let pitch = mapped.pitch as usize;
        for y in 0..height as usize {
            out.extend_from_slice(std::slice::from_raw_parts(mapped.bits.add(y * pitch), row));
        }
        staging.Unmap()?;
    }
    Ok(out)
}

/// A WARP device, a target bitmap and a painter kept across frames, so tests can see what the
/// painter caches between them. Fields drop in order: the painter before the device.
pub struct Offscreen {
    painter: Painter,
    target: ID2D1Bitmap1,
    staging: ID2D1Bitmap1,
    width: u32,
    height: u32,
    gpu: Gpu,
}

impl Offscreen {
    /// A `width_px` by `height_px` target at `scale`, painted with `palette` and `text`.
    ///
    /// # Errors
    /// Returns the first failing Windows call, or `E_INVALIDARG` for a zero-sized target.
    pub fn new(
        width_px: u32,
        height_px: u32,
        scale: f64,
        palette: Palette,
        text: TextSystem,
    ) -> Result<Self> {
        if width_px == 0 || height_px == 0 {
            return Err(Error::from(E_INVALIDARG));
        }
        let gpu = Gpu::new(true)?;
        let ctx = gpu.context();
        #[allow(clippy::cast_possible_truncation)]
        let dpi = (96.0 * scale) as f32;
        let target = make_bitmap(ctx, width_px, height_px, dpi, D2D1_BITMAP_OPTIONS_TARGET)?;
        let staging = make_bitmap(
            ctx,
            width_px,
            height_px,
            dpi,
            D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        )?;
        // SAFETY: `target` is a live target bitmap created on this context.
        unsafe {
            ctx.SetTarget(&target);
            ctx.SetDpi(dpi, dpi);
        }
        let painter = Painter::new(ctx, gpu.d2d_factory(), palette, text)?;
        Ok(Self {
            painter,
            target,
            staging,
            width: width_px,
            height: height_px,
            gpu,
        })
    }

    /// Paints `frame` and returns tightly packed premultiplied BGRA rows (`width_px * 4` bytes
    /// each).
    ///
    /// # Errors
    /// Returns the first failing Windows call.
    pub fn render(&mut self, frame: &PaintFrame) -> Result<Vec<u8>> {
        self.painter.paint(self.gpu.context(), frame)?;
        read_back(&self.staging, &self.target, self.width, self.height)
    }

    /// Switches the theme: both palettes and the material, if any.
    pub fn set_theme(&mut self, paint: ThemePaint) {
        self.painter.set_theme(paint);
    }

    /// How many page or pill bitmaps the painter redrew so far.
    #[must_use]
    pub fn page_rebuilds(&self) -> u64 {
        self.painter.page_rebuilds()
    }

    /// How many times the painter built the material's bitmaps so far.
    #[must_use]
    pub fn material_builds(&self) -> u64 {
        self.painter.material_builds()
    }

    /// How many times the painter built the island geometry so far.
    #[must_use]
    pub fn island_builds(&self) -> u64 {
        self.painter.island_builds()
    }
}

impl Drop for Offscreen {
    fn drop(&mut self) {
        // SAFETY: detaches the target before the bitmap and the context are released.
        unsafe { self.gpu.context().SetTarget(None::<&ID2D1Image>) };
    }
}

/// Paints `frame` with `palette` and `text` into a `width_px` by `height_px` WARP bitmap at
/// `scale` and returns tightly packed premultiplied BGRA rows (`width_px * 4` bytes each).
///
/// # Errors
/// Returns the first failing Windows call, or `E_INVALIDARG` for a zero-sized target.
pub fn render_offscreen(
    width_px: u32,
    height_px: u32,
    scale: f64,
    frame: &PaintFrame,
    palette: Palette,
    text: TextSystem,
) -> Result<Vec<u8>> {
    Offscreen::new(width_px, height_px, scale, palette, text)?.render(frame)
}
