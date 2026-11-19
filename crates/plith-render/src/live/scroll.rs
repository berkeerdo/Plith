//! The pill's one-shot title scroll: the whole title moved by the scroll offset, clipped to its
//! box with the trailing edge faded while it overflows. A title that fits is drawn plainly, with
//! no layer.

use crate::list::ListKit;
use crate::paint::{color, f};
use crate::text::TextSystem;
use plith_core::draw::{Rect, TextStyle};
use plith_core::scroll::scroll_offset_in_run;
use std::time::SystemTime;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_GRADIENT_STOP};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_EXTEND_MODE_CLAMP,
    D2D1_GAMMA_2_2, D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, ID2D1Brush, ID2D1DeviceContext, ID2D1Layer,
    ID2D1LinearGradientBrush, ID2D1RenderTarget,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_TEXT_ALIGNMENT_LEADING, IDWriteTextLayout};
use windows::core::Result;
use windows_numerics::Vector2;

/// The length of the fade at the trailing edge, in DIP.
pub(crate) const FADE_W: f64 = 16.0;

/// One scroll node to draw.
pub(super) struct ScrollNode<'a> {
    /// The visible box.
    pub(super) rect: Rect,
    /// The whole title.
    pub(super) text: &'a str,
    /// The text style.
    pub(super) style: TextStyle,
    /// The resolved ink.
    pub(super) rgba: [f32; 4],
    /// When the run began.
    pub(super) start: SystemTime,
    /// The planned run length in seconds.
    pub(super) run_s: f64,
}

/// The fade's opacity mask: opaque up to its start point, clear at its end point. Created once
/// with the list kit; its points are moved per frame.
///
/// # Errors
/// Returns the failing gradient call.
pub(crate) fn fade_brush(ctx: &ID2D1DeviceContext) -> Result<ID2D1LinearGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: color([0.0, 0.0, 0.0, 1.0], 1.0),
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: color([0.0, 0.0, 0.0, 0.0], 1.0),
        },
    ];
    let target: &ID2D1RenderTarget = ctx;
    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES::default();
    // SAFETY: `stops` and `props` are live for the calls.
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        target.CreateLinearGradientBrush(&raw const props, None, &collection)
    }
}

/// Draws `node` at `now` in the kit brush's group alpha. A failed layout loses this frame's
/// title only; the next frame retries.
pub(super) fn scroll(
    ctx: &ID2D1DeviceContext,
    (kit, text): (&ListKit, &mut TextSystem),
    node: &ScrollNode<'_>,
    now: SystemTime,
) {
    let Ok((layout, width)) = text.line(node.text, node.style) else {
        return;
    };
    let rect = node.rect;
    kit.set_ink(node.rgba);
    let overflow = f64::from(width) - rect.w;
    if overflow <= 0.0 {
        draw_at(ctx, kit, &layout, (rect.x, rect.y));
        return;
    }
    // Before the start (or a default wall time) the elapsed time is 0: at rest.
    let elapsed = now
        .duration_since(node.start)
        .map_or(0.0, |since| since.as_secs_f64());
    let offset = scroll_offset_in_run(elapsed, node.run_s, overflow);
    push_fade(ctx, &kit.fade, rect);
    draw_at(ctx, kit, &layout, (rect.x + offset, rect.y));
    // SAFETY: pairs the PushLayer of `push_fade`, inside the frame's BeginDraw/EndDraw.
    unsafe { ctx.PopLayer() };
}

/// Draws `layout` from its leading edge at `(x, y)`.
fn draw_at(
    ctx: &ID2D1DeviceContext,
    kit: &ListKit,
    layout: &IDWriteTextLayout,
    (x, y): (f64, f64),
) {
    let origin = Vector2 { X: f(x), Y: f(y) };
    // SAFETY: the layout is live; the draw call is inside the frame's BeginDraw/EndDraw.
    unsafe {
        if layout
            .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)
            .is_ok()
        {
            ctx.DrawTextLayout(origin, layout, &kit.brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
        }
    }
}

/// Pushes a layer clipped to `rect` horizontally (with a box's height of room above and below
/// for the glyphs) whose opacity falls to 0 over the last `FADE_W` DIP of `rect`.
fn push_fade(ctx: &ID2D1DeviceContext, fade: &ID2D1LinearGradientBrush, rect: Rect) {
    let right = rect.x + rect.w;
    let y = rect.y + rect.h / 2.0;
    let brush: ID2D1Brush = fade.clone().into();
    // SAFETY: plain setters on a live brush; the brush is moved into the layer parameters and
    // released by the explicit drop below, after PushLayer took its own reference.
    unsafe {
        fade.SetStartPoint(Vector2 {
            X: f(right - FADE_W),
            Y: f(y),
        });
        fade.SetEndPoint(Vector2 {
            X: f(right),
            Y: f(y),
        });
        let params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F {
                left: f(rect.x),
                top: f(rect.y - rect.h),
                right: f(right),
                bottom: f(rect.y + 2.0 * rect.h),
            },
            geometricMask: std::mem::ManuallyDrop::new(None),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: windows_numerics::Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: std::mem::ManuallyDrop::new(Some(brush)),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        ctx.PushLayer(&raw const params, None::<&ID2D1Layer>);
        drop(std::mem::ManuallyDrop::into_inner(params.opacityBrush));
    }
}
