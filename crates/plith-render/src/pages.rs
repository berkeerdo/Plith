//! The page cache: each page's display list (and the pill's) drawn once into its own bitmap and
//! redrawn only when the list, the scale or the palette changes, so text is rasterized once per
//! content change and a frame only composites bitmaps.

use crate::list::{ListEnv, ListKit, draw_list};
use crate::paint::{color, f};
use plith_core::draw::{ArtId, DisplayList};
use plith_core::theme::Palette;
use std::sync::Arc;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
    ID2D1Bitmap1, ID2D1DeviceContext,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;

/// The cache slot of the pill; slots 0 to 3 are the pages.
pub(crate) const PILL: usize = 4;
/// The largest bitmap side in pixels; a list larger than this is clipped.
const MAX_PX: f64 = 8192.0;

/// One drawn list and what it was drawn for.
struct Entry {
    list: Arc<DisplayList>,
    scale: u64,
    palette: Palette,
    /// `None` for a list with no area.
    bitmap: Option<ID2D1Bitmap1>,
    /// Artwork the list referenced that was not there yet.
    missing: Vec<ArtId>,
}

/// One bitmap per page and one for the pill.
#[derive(Default)]
pub(crate) struct PageCache {
    slots: [Option<Entry>; PILL + 1],
    rebuilds: u64,
}

/// Pixels for `dip` at `scale`, at least 1 and at most `MAX_PX`; `None` for no area.
// Allowed: the value is ceiled and clamped to 1..=8192 first.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixels(dip: f64, scale: f64) -> Option<u32> {
    let px = (dip * scale).ceil();
    (px.is_finite() && px >= 1.0).then(|| px.min(MAX_PX) as u32)
}

fn target_bitmap(ctx: &ID2D1DeviceContext, size: D2D_SIZE_U, scale: f64) -> Result<ID2D1Bitmap1> {
    let dpi = f(96.0 * scale);
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
        colorContext: std::mem::ManuallyDrop::new(None),
    };
    // SAFETY: `props` is live for the call; no initial data is passed.
    unsafe { ctx.CreateBitmap(size, None, 0, &raw const props) }
}

impl Entry {
    fn is_fresh(&self, list: &Arc<DisplayList>, scale: f64, env: &ListEnv<'_>) -> bool {
        Arc::ptr_eq(&self.list, list)
            && self.scale == scale.to_bits()
            && self.palette == *env.palette
            && !self
                .missing
                .iter()
                .any(|id| env.art.iter().any(|art| art.id == *id))
    }
}

impl PageCache {
    /// How many bitmaps were redrawn so far.
    pub(crate) fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    /// The bitmap of `list` in `slot`, redrawn only when the list `Arc`, `env.scale` or the
    /// palette changed (or missing artwork arrived). `None` for a list with no area. Must be
    /// called outside the frame's `BeginDraw`: it draws with the context's target switched to
    /// the bitmap and restores the target afterwards.
    ///
    /// # Errors
    /// Returns the failing bitmap creation, draw or `EndDraw` call.
    pub(crate) fn draw(
        &mut self,
        ctx: &ID2D1DeviceContext,
        slot: usize,
        list: &Arc<DisplayList>,
        kit: &mut ListKit,
        env: &mut ListEnv<'_>,
    ) -> Result<Option<ID2D1Bitmap1>> {
        let scale = env.scale;
        let Some(cell) = self.slots.get_mut(slot) else {
            return Ok(None);
        };
        if let Some(entry) = cell.as_ref().filter(|e| e.is_fresh(list, scale, env)) {
            return Ok(entry.bitmap.clone());
        }
        let old = cell.take().and_then(|e| e.bitmap);
        let size = pixels(list.size.0, scale)
            .zip(pixels(list.size.1, scale))
            .map(|(width, height)| D2D_SIZE_U { width, height });
        let (bitmap, missing) = match size {
            Some(size) => {
                // SAFETY: plain getter on a live bitmap.
                let reusable = old.filter(|b| unsafe { b.GetPixelSize() } == size);
                let bitmap = match reusable {
                    Some(bitmap) => bitmap,
                    None => target_bitmap(ctx, size, scale)?,
                };
                let missing = render_into(ctx, &bitmap, list, kit, env)?;
                (Some(bitmap), missing)
            }
            None => (None, Vec::new()),
        };
        self.rebuilds += 1;
        *cell = Some(Entry {
            list: Arc::clone(list),
            scale: scale.to_bits(),
            palette: *env.palette,
            bitmap: bitmap.clone(),
            missing,
        });
        Ok(bitmap)
    }
}

/// Draws `list` into `bitmap` between its own `BeginDraw`/`EndDraw`, with grayscale text (the
/// bitmap is transparent, so `ClearType` cannot blend), then restores the context's target and
/// text mode.
fn render_into(
    ctx: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    list: &DisplayList,
    kit: &mut ListKit,
    env: &mut ListEnv<'_>,
) -> Result<Vec<ArtId>> {
    let clear = color([0.0; 4], 1.0);
    // SAFETY: plain COM calls on live objects; this runs outside any other BeginDraw.
    let (previous, text_mode) = unsafe {
        let previous = ctx.GetTarget().ok();
        let text_mode = ctx.GetTextAntialiasMode();
        ctx.SetTarget(bitmap);
        ctx.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        ctx.BeginDraw();
        ctx.Clear(Some(&raw const clear));
        (previous, text_mode)
    };
    let drawn = draw_list(ctx, kit, env, list);
    // SAFETY: closes the BeginDraw above and restores the saved state.
    let ended = unsafe {
        let ended = ctx.EndDraw(None, None);
        ctx.SetTextAntialiasMode(text_mode);
        ctx.SetTarget(previous.as_ref());
        ended
    };
    let missing = drawn?;
    ended?;
    Ok(missing)
}
