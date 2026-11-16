//! Live nodes, drawn every frame on top of the incoming page from the sampled values: meters,
//! level traces and the seek fill. Group opacity scales each brush colour's alpha (no layer).

use crate::list::ListKit;
use crate::paint::f;
use plith_core::ambient::LiveValues;
use plith_core::draw::{DisplayList, LiveNode, Node, Rect, SeekTimeline, StationMark};
use plith_core::theme::Palette;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows_numerics::Vector2;

/// A level at or below this draws no meter.
const SILENT: f32 = 0.001;
/// The seek line's width in DIP.
const SEEK_W: f32 = 2.0;
/// A trace line's width in DIP.
const TRACE_W: f32 = 1.0;

/// Draws the live nodes of `list` in page coordinates (the caller sets the page's offset as
/// the context transform), with every colour's alpha scaled by `alpha`. Inside a `BeginDraw`.
pub(crate) fn draw_live(
    ctx: &ID2D1DeviceContext,
    kit: &mut ListKit,
    list: &DisplayList,
    live: &LiveValues,
    palette: &Palette,
    alpha: f32,
) {
    kit.set_color(palette.flow, alpha);
    for node in &list.nodes {
        let Node::Live(node) = node else {
            continue;
        };
        match node {
            LiveNode::Meter { rect, rail, index } => {
                meter(ctx, kit, *rect, live.peak(*rail, *index), palette);
            }
            LiveNode::Trace { rect, index, .. } => {
                trace(ctx, kit, *rect, live.trace(*index), palette);
            }
            LiveNode::Seek { rect, timeline } => seek(ctx, kit, *rect, timeline, live, palette),
        }
    }
}

/// A bar from the left of `rect` over the level's share of its width, in the flow colour.
fn meter(ctx: &ID2D1DeviceContext, kit: &ListKit, rect: Rect, peak: f32, palette: &Palette) {
    if peak.is_nan() || peak <= SILENT {
        return;
    }
    let level = Rect {
        w: rect.w * f64::from(peak.clamp(0.0, 1.0)),
        ..rect
    };
    kit.fill(ctx, level, 0.0, palette.flow);
}

/// The samples (0 to 1, oldest first) spread across `rect`, bottom-aligned, as 1 DIP track
/// segments drawn with `DrawLine` (no path geometry per frame). Fewer than two samples draw
/// nothing.
fn trace(ctx: &ID2D1DeviceContext, kit: &ListKit, rect: Rect, samples: &[f32], palette: &Palette) {
    if samples.len() < 2 {
        return;
    }
    // Allowed: sample counts are small, exact in f64.
    #[allow(clippy::cast_precision_loss)]
    let step = rect.w / (samples.len() - 1) as f64;
    let bottom = rect.y + rect.h;
    let at = |i: usize, v: f32| {
        // Allowed: as above.
        #[allow(clippy::cast_precision_loss)]
        let x = rect.x + step * i as f64;
        Vector2 {
            X: f(x),
            Y: f(bottom - f64::from(v.clamp(0.0, 1.0)) * rect.h),
        }
    };
    kit.set_ink(palette.track);
    for (i, pair) in samples.windows(2).enumerate() {
        // SAFETY: a draw call inside the frame's BeginDraw/EndDraw.
        unsafe {
            ctx.DrawLine(
                at(i, pair[0]),
                at(i + 1, pair[1]),
                &kit.brush,
                TRACE_W,
                None,
            );
        }
    }
}

/// The played part of the track as a 2 DIP flow line along the middle of `rect`, with a
/// filled ink station at its head.
fn seek(
    ctx: &ID2D1DeviceContext,
    kit: &ListKit,
    rect: Rect,
    timeline: &SeekTimeline,
    live: &LiveValues,
    palette: &Palette,
) {
    let played = timeline.fraction_at(live.now);
    let y = rect.y + rect.h / 2.0;
    let head = rect.x + rect.w * played;
    if head > rect.x {
        kit.set_ink(palette.flow);
        let from = Vector2 {
            X: f(rect.x),
            Y: f(y),
        };
        let to = Vector2 {
            X: f(head),
            Y: f(y),
        };
        // SAFETY: a draw call inside the frame's BeginDraw/EndDraw.
        unsafe { ctx.DrawLine(from, to, &kit.brush, SEEK_W, None) };
    }
    kit.station(
        ctx,
        (head, y),
        StationMark::Filled,
        palette.ink,
        palette.floor_high,
    );
}
