//! Draws a sheet display list on an opaque floor, into the window or an offscreen bitmap.

use crate::device::Gpu;
use crate::list::{ListEnv, ListKit, draw_list};
use crate::offscreen::{make_bitmap, read_back};
use crate::paint::color;
use crate::text::TextSystem;
use plith_core::draw::{ArtPixels, DisplayList};
use plith_core::theme::Palette;
use std::sync::Arc;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1DeviceContext, ID2D1Image,
};
use windows::core::{Error, Result};
use windows_numerics::Matrix3x2;

/// The sheet's brushes, text system and colours. Every object belongs to one device context.
pub(crate) struct SheetPainter {
    kit: ListKit,
    text: TextSystem,
    palette: Palette,
    /// Artwork `Node::Image` may name; fixed for the window's life.
    art: Vec<Arc<ArtPixels>>,
}

impl SheetPainter {
    /// Creates the brushes on `gpu`'s context and a text system of its own; `art` is the
    /// artwork the lists may name.
    ///
    /// # Errors
    /// Returns the failing brush or DirectWrite call.
    pub(crate) fn new(gpu: &Gpu, palette: Palette, art: Vec<Arc<ArtPixels>>) -> Result<Self> {
        Ok(Self {
            kit: ListKit::new(gpu.context(), gpu.d2d_factory())?,
            text: TextSystem::new()?,
            palette,
            art,
        })
    }

    /// Switches the colours used by the next `draw`.
    pub(crate) fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    /// Clears the context's target to the opaque floor and draws `list` at `scale` (the
    /// context's DIP scale) in one `BeginDraw`/`EndDraw`. Text is grayscale, as on the island.
    ///
    /// # Errors
    /// Returns the first failing draw call or the `EndDraw` error (for example device removed).
    pub(crate) fn draw(
        &mut self,
        ctx: &ID2D1DeviceContext,
        list: &DisplayList,
        scale: f64,
    ) -> Result<()> {
        let mut floor = self.palette.floor;
        floor[3] = 1.0;
        let clear = color(floor, 1.0);
        let identity = Matrix3x2::identity();
        // SAFETY: state and clear calls on a live context with a target, after BeginDraw.
        unsafe {
            ctx.BeginDraw();
            ctx.SetTransform(&raw const identity);
            ctx.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            ctx.Clear(Some(&raw const clear));
        }
        let mut env = ListEnv {
            palette: &self.palette,
            text: &mut self.text,
            art: &self.art,
            scale,
        };
        let drawn = draw_list(ctx, &mut self.kit, &mut env, list);
        // SAFETY: closes the BeginDraw above.
        let ended = unsafe { ctx.EndDraw(None, None) };
        drawn?;
        ended
    }
}

/// The pixel size of `list` at `scale`, rounded up.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn pixel_size(list: &DisplayList, scale: f64) -> (u32, u32) {
    // Sheet sizes are a few hundred DIP; negative or NaN become 0 and are refused.
    let px = |dip: f64| (dip * scale).ceil().max(0.0) as u32;
    (px(list.size.0), px(list.size.1))
}

/// Draws `list` on a WARP device into a bitmap of its size at `scale` and returns tightly packed
/// premultiplied BGRA rows.
///
/// # Errors
/// Returns the first failing Windows call, or `E_INVALIDARG` for an empty list size.
pub(crate) fn offscreen(
    list: &DisplayList,
    scale: f64,
    palette: Palette,
    art: &[Arc<ArtPixels>],
) -> Result<Vec<u8>> {
    let (w, h) = pixel_size(list, scale);
    if w == 0 || h == 0 {
        return Err(Error::from(E_INVALIDARG));
    }
    let gpu = Gpu::new(true)?;
    let ctx = gpu.context();
    #[allow(clippy::cast_possible_truncation)]
    let dpi = (96.0 * scale) as f32;
    let target = make_bitmap(ctx, w, h, dpi, D2D1_BITMAP_OPTIONS_TARGET)?;
    let staging = make_bitmap(
        ctx,
        w,
        h,
        dpi,
        D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    )?;
    // SAFETY: `target` is a live target bitmap created on this context.
    unsafe {
        ctx.SetTarget(&target);
        ctx.SetDpi(dpi, dpi);
    }
    let drawn =
        SheetPainter::new(&gpu, palette, art.to_vec()).and_then(|mut p| p.draw(ctx, list, scale));
    let rows = drawn.and_then(|()| read_back(&staging, &target, w, h));
    // SAFETY: detaches the target before the bitmap and the context are released.
    unsafe { ctx.SetTarget(None::<&ID2D1Image>) };
    rows
}
