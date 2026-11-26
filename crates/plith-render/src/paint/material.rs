//! The material floor (spec `2026-11-26-halka-island-design.md`, sections 3 and 5): the glossy
//! pill three-sliced over the shape, the fabric revealed in a circle from the source centre,
//! then the ripple rings and the source disc while a motion runs. Drawn inside the shape's clip.
//!
//! The bitmaps are made once per art (`set` with a new `Arc`) on the first frame that needs
//! them; a frame only moves the fabric brush and issues draw calls.

use super::{f, push_clip, snap};
use plith_core::draw::ArtPixels;
use plith_core::geometry::IslandShape;
use plith_core::material::MaterialLayers;
use plith_core::material::art::MaterialArt;
use plith_core::material::place::{
    DISC_AT, FABRIC_ANCHOR, FABRIC_W, PILL_H, RING_AT, cap_opacity, reveal_radius, source_point,
    three_slice,
};
use std::sync::Arc;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES1, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, D2D1_ELLIPSE,
    D2D1_EXTEND_MODE_MIRROR, D2D1_INTERPOLATION_MODE_LINEAR, ID2D1Bitmap1, ID2D1BitmapBrush1,
    ID2D1DeviceContext,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::{Error, Result};
use windows_numerics::{Matrix3x2, Vector2};

/// The device objects of one art set.
struct Bitmaps {
    fabric: ID2D1BitmapBrush1,
    rings: [ID2D1Bitmap1; 3],
    disc: ID2D1Bitmap1,
    pill: ID2D1Bitmap1,
}

/// The current material and its bitmaps, made lazily on the render thread.
#[derive(Default)]
pub(super) struct Material {
    art: Option<Arc<MaterialArt>>,
    bitmaps: Option<Bitmaps>,
    builds: u64,
}

/// A device bitmap of `pixels` at `dpi` (so its DIP size is the art's, whatever the file scale).
fn bitmap(ctx: &ID2D1DeviceContext, pixels: &ArtPixels, dpi: f32) -> Result<ID2D1Bitmap1> {
    let pitch = pixels.width.saturating_mul(4);
    let needed = u64::from(pitch) * u64::from(pixels.height);
    if pixels.width == 0 || pixels.height == 0 || (pixels.bgra.len() as u64) < needed {
        return Err(Error::from(E_INVALIDARG));
    }
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
        colorContext: std::mem::ManuallyDrop::new(None),
    };
    let size = D2D_SIZE_U {
        width: pixels.width,
        height: pixels.height,
    };
    // SAFETY: the buffer holds at least `pitch * height` bytes (checked above) and outlives the
    // call, which copies it; `props` is live.
    unsafe {
        ctx.CreateBitmap(
            size,
            Some(pixels.bgra.as_ptr().cast()),
            pitch,
            &raw const props,
        )
    }
}

fn build(ctx: &ID2D1DeviceContext, art: &MaterialArt) -> Result<Bitmaps> {
    // Every file of a set shares one scale class: the fabric's width over its DIP width.
    let dpi = f(96.0 * f64::from(art.fabric.width) / FABRIC_W);
    let fabric = bitmap(ctx, &art.fabric, dpi)?;
    let props = D2D1_BITMAP_BRUSH_PROPERTIES1 {
        extendModeX: D2D1_EXTEND_MODE_MIRROR,
        extendModeY: D2D1_EXTEND_MODE_MIRROR,
        interpolationMode: D2D1_INTERPOLATION_MODE_LINEAR,
    };
    // SAFETY: the bitmap and `props` are live for the call.
    let fabric = unsafe { ctx.CreateBitmapBrush(&fabric, Some(&raw const props), None)? };
    Ok(Bitmaps {
        fabric,
        rings: [
            bitmap(ctx, &art.rings[0], dpi)?,
            bitmap(ctx, &art.rings[1], dpi)?,
            bitmap(ctx, &art.rings[2], dpi)?,
        ],
        disc: bitmap(ctx, &art.disc, dpi)?,
        pill: bitmap(ctx, &art.pill, dpi)?,
    })
}

fn rect(left: f64, top: f64, width: f64, height: f64) -> D2D_RECT_F {
    D2D_RECT_F {
        left: f(left),
        top: f(top),
        right: f(left + width),
        bottom: f(top + height),
    }
}

/// Draws `bitmap` (or its `src` part) into `dest` at `opacity`.
fn blit(
    ctx: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    dest: D2D_RECT_F,
    src: Option<D2D_RECT_F>,
    opacity: f32,
) {
    let src = src.as_ref().map(std::ptr::from_ref);
    // SAFETY: a draw call inside the frame's BeginDraw/EndDraw on a live bitmap; the rects
    // are live locals.
    unsafe {
        ctx.DrawBitmap(
            bitmap,
            Some(&raw const dest),
            opacity,
            D2D1_INTERPOLATION_MODE_LINEAR,
            src,
            None,
        );
    }
}

/// The fabric brush's top-left: the source centre minus the fabric's anchor, snapped to device
/// pixels.
fn fabric_origin((sx, sy): (f64, f64), scale: f64) -> (f64, f64) {
    (
        snap(sx - FABRIC_ANCHOR.0, scale),
        snap(sy - FABRIC_ANCHOR.1, scale),
    )
}

/// An overlay's top-left, `at` from the source centre, measured from the snapped fabric origin
/// and not snapped again: the overlays then resample with the fabric's own phase at any scale
/// (a separate snap moves them up to half a device pixel against the weave at 1.25 or 1.75).
fn overlay_origin(source: (f64, f64), at: (f64, f64), scale: f64) -> (f64, f64) {
    let (fx, fy) = fabric_origin(source, scale);
    (fx + FABRIC_ANCHOR.0 + at.0, fy + FABRIC_ANCHOR.1 + at.1)
}

/// An overlay (a ring or the disc) at `at` from the source centre.
fn overlay(
    ctx: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    source: (f64, f64),
    at: (f64, f64),
    opacity: f32,
    scale: f64,
) {
    if opacity <= 0.0 {
        return;
    }
    // SAFETY: plain getter on a live bitmap.
    let size = unsafe { bitmap.GetSize() };
    let (left, top) = overlay_origin(source, at, scale);
    let dest = rect(left, top, f64::from(size.width), f64::from(size.height));
    blit(ctx, bitmap, dest, None, opacity);
}

impl Material {
    /// Takes the theme's art; a different `Arc` (or `None`) drops the bitmaps of the old one.
    pub(super) fn set(&mut self, art: Option<Arc<MaterialArt>>) {
        let same = match (&self.art, &art) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.bitmaps = None;
        }
        self.art = art;
    }

    /// Whether the theme has a material.
    pub(super) fn is_set(&self) -> bool {
        self.art.is_some()
    }

    /// How many bitmap sets were built.
    pub(super) fn builds(&self) -> u64 {
        self.builds
    }

    /// Makes the bitmaps of the current art if they are missing.
    ///
    /// # Errors
    /// Returns the failing bitmap or brush creation.
    pub(super) fn ready(&mut self, ctx: &ID2D1DeviceContext) -> Result<()> {
        if let (Some(art), None) = (&self.art, &self.bitmaps) {
            self.bitmaps = Some(build(ctx, art)?);
            self.builds += 1;
        }
        Ok(())
    }

    /// Draws the floor of `shape` at `origin` with `layers`, bottom to top: pill, fabric
    /// reveal, rings, disc. Inside the frame's `BeginDraw` and the shape's clip; nothing before
    /// `ready`.
    pub(super) fn draw(
        &self,
        ctx: &ID2D1DeviceContext,
        shape: IslandShape,
        origin: (f64, f64),
        layers: &MaterialLayers,
        scale: f64,
    ) {
        let Some(b) = &self.bitmaps else {
            return;
        };
        if layers.pill > 0.0 {
            draw_pill(ctx, &b.pill, shape, origin, layers.pill, scale);
        }
        let source = source_point(shape, origin);
        if layers.reveal > 0.0 {
            draw_fabric(ctx, &b.fabric, shape, origin, layers.reveal, scale);
        }
        // The rings are pressed into the fabric: under a partial reveal they are clipped to its
        // circle (their rasters carry fabric, which must not show on the black base). Without
        // the clip's geometry they are left out for the frame.
        let partial = layers.reveal < 1.0;
        let shown = layers.ripples.iter().any(|r| *r > 0.0)
            && (!partial || push_reveal(ctx, shape, source, layers.reveal));
        if shown {
            for (ring, (opacity, at)) in b.rings.iter().zip(layers.ripples.iter().zip(RING_AT)) {
                overlay(ctx, ring, source, at, *opacity, scale);
            }
            if partial {
                // SAFETY: pairs the PushLayer of `push_reveal`, inside the frame's BeginDraw.
                unsafe { ctx.PopLayer() };
            }
        }
        let disc_at = (DISC_AT.0, DISC_AT.1 - layers.disc_lift);
        overlay(ctx, &b.disc, source, disc_at, layers.disc, scale);
    }
}

/// The pill three-sliced over the shape's width at its own height, its top on the island's top:
/// a shorter island crops it, a taller one shows the opaque base below it (no vertical stretch).
/// Below the pill's height the caps fade out (`cap_opacity`) over the straight middle drawn
/// across the whole width, so the Hidden sliver is the pill's black top end to end.
fn draw_pill(
    ctx: &ID2D1DeviceContext,
    pill: &ID2D1Bitmap1,
    shape: IslandShape,
    (ox, oy): (f64, f64),
    opacity: f32,
    scale: f64,
) {
    // SAFETY: plain getter on a live bitmap.
    let size = unsafe { pill.GetSize() };
    let art_h = f64::from(size.height);
    let (top, bottom) = (snap(oy, scale), snap(oy + PILL_H, scale));
    let span = |left: f64, width: f64, src: (f64, f64), alpha: f32| {
        if width <= 0.0 || src.1 <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (l, r) = (snap(left, scale), snap(left + width, scale));
        let dest = rect(l, top, r - l, bottom - top);
        blit(ctx, pill, dest, Some(rect(src.0, 0.0, src.1, art_h)), alpha);
    };
    let [left_cap, middle, right_cap] = three_slice(f64::from(size.width), ox, shape.width);
    let caps = f(cap_opacity(shape.height));
    if caps < 1.0 {
        span(ox, shape.width, middle.src, opacity);
    } else {
        span(middle.dst.0, middle.dst.1, middle.src, opacity);
    }
    for cap in [left_cap, right_cap] {
        span(cap.dst.0, cap.dst.1, cap.src, opacity * caps);
    }
}

/// Pushes a layer clipped to the reveal circle of `shape` at `reveal`; false (and nothing
/// pushed) when the circle's geometry cannot be made.
fn push_reveal(
    ctx: &ID2D1DeviceContext,
    shape: IslandShape,
    (sx, sy): (f64, f64),
    reveal: f64,
) -> bool {
    let r = f(reveal_radius(shape, reveal));
    let circle = D2D1_ELLIPSE {
        point: Vector2 { X: f(sx), Y: f(sy) },
        radiusX: r,
        radiusY: r,
    };
    // SAFETY: plain factory calls on the live context; `circle` is a live local.
    let geometry = unsafe {
        ctx.GetFactory()
            .and_then(|factory| factory.CreateEllipseGeometry(&raw const circle))
    };
    match geometry {
        Ok(geometry) => {
            push_clip(ctx, geometry.into());
            true
        }
        Err(_) => false,
    }
}

/// The fabric over the reveal circle (the whole shape at a full reveal), anchored so its
/// source centre lands on the island's, mirrored past its edges.
fn draw_fabric(
    ctx: &ID2D1DeviceContext,
    fabric: &ID2D1BitmapBrush1,
    shape: IslandShape,
    origin: (f64, f64),
    reveal: f64,
    scale: f64,
) {
    let (sx, sy) = source_point(shape, origin);
    let (fx, fy) = fabric_origin((sx, sy), scale);
    let shift = Matrix3x2::translation(f(fx), f(fy));
    // SAFETY: plain state change and draw calls inside the frame's BeginDraw/EndDraw on a live
    // brush; the transform and shapes are live locals.
    unsafe {
        fabric.SetTransform(&raw const shift);
        if reveal >= 1.0 {
            let whole = rect(origin.0, origin.1, shape.width, shape.height);
            ctx.FillRectangle(&raw const whole, fabric);
        } else {
            let r = f(reveal_radius(shape, reveal));
            let circle = D2D1_ELLIPSE {
                point: Vector2 { X: f(sx), Y: f(sy) },
                radiusX: r,
                radiusY: r,
            };
            ctx.FillEllipse(&raw const circle, fabric);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlays_sit_on_the_fabric_grid_at_any_scale() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            for source in [(310.0, 33.0), (310.3, 33.4), (311.7, 34.9)] {
                let fabric = fabric_origin(source, scale);
                for at in RING_AT.into_iter().chain([DISC_AT]) {
                    let o = overlay_origin(source, at, scale);
                    let want = (FABRIC_ANCHOR.0 + at.0, FABRIC_ANCHOR.1 + at.1);
                    let got = (o.0 - fabric.0, o.1 - fabric.1);
                    assert!(
                        (got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9,
                        "scale {scale}, source {source:?}, at {at:?}: {got:?} vs {want:?}"
                    );
                }
            }
        }
    }
}
