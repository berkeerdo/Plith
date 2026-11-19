//! Live nodes, drawn every frame on top of the incoming page from the sampled values: meters,
//! level traces, the seek fill and the elapsed time; and on the pill, the title scroll (`scroll`).
//! Group opacity scales each brush colour's alpha (no layer).

mod scroll;

pub(crate) use scroll::fade_brush;

use crate::list::ListKit;
use crate::paint::f;
use crate::text::TextSystem;
use plith_core::ambient::LiveValues;
use plith_core::draw::{DisplayList, Ink, LiveNode, Node, Rect, SeekTimeline, StationMark, Trim};
use plith_core::pages::media::elapsed_text;
use plith_core::theme::Palette;
use scroll::{ScrollNode, scroll};
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
/// `text` lays out the elapsed time.
pub(crate) fn draw_live(
    ctx: &ID2D1DeviceContext,
    (kit, text): (&mut ListKit, &mut TextSystem),
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
            LiveNode::Seek {
                rect,
                timeline,
                ink,
            } => seek(ctx, kit, (*rect, timeline, *ink), live, palette),
            LiveNode::Elapsed {
                rect,
                timeline,
                style,
                ink,
                align,
            } => {
                let shown = elapsed_text(timeline, live.now);
                // The brush keeps the group alpha set above. A failed layout loses this frame's
                // elapsed text only; the next frame retries.
                let node = (*rect, shown.as_str(), *style, *align, Trim::End);
                let _ = kit.text(ctx, text, node, palette.resolve(*ink));
            }
            LiveNode::Scroll {
                rect,
                text: title,
                style,
                ink,
                start,
                run_s,
            } => {
                let node = ScrollNode {
                    rect: *rect,
                    text: title,
                    style: *style,
                    rgba: palette.resolve(*ink),
                    start: *start,
                    run_s: *run_s,
                };
                scroll(ctx, (kit, text), &node, live.now);
            }
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

/// The mock's trace: a 1 DIP faint-track baseline 1 DIP inside the bottom of `rect`, and the
/// samples (0 to 1, oldest first) spread across `rect` as faint 1 DIP segments rising up to
/// `h - 2` above it, drawn with `DrawLine` (no path geometry per frame). Fewer than two samples
/// draw only the baseline.
fn trace(ctx: &ID2D1DeviceContext, kit: &ListKit, rect: Rect, samples: &[f32], palette: &Palette) {
    let bottom = rect.y + rect.h - 1.0;
    let swing = (rect.h - 2.0).max(0.0);
    kit.set_ink(palette.faint_track);
    let (left, right) = (
        Vector2 {
            X: f(rect.x),
            Y: f(bottom),
        },
        Vector2 {
            X: f(rect.x + rect.w),
            Y: f(bottom),
        },
    );
    // SAFETY: a draw call inside the frame's BeginDraw/EndDraw.
    unsafe { ctx.DrawLine(left, right, &kit.brush, TRACE_W, None) };
    if samples.len() < 2 {
        return;
    }
    // Allowed: sample counts are small, exact in f64.
    #[allow(clippy::cast_precision_loss)]
    let step = rect.w / (samples.len() - 1) as f64;
    let at = |i: usize, v: f32| {
        // Allowed: as above.
        #[allow(clippy::cast_precision_loss)]
        let x = rect.x + step * i as f64;
        Vector2 {
            X: f(x),
            Y: f(bottom - f64::from(v.clamp(0.0, 1.0)) * swing),
        }
    };
    kit.set_ink(palette.faint);
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

/// The played part of the track as a 2 DIP line along the middle of `rect` with a filled
/// station at its head, in the ink the node carries (core picks it).
fn seek(
    ctx: &ID2D1DeviceContext,
    kit: &ListKit,
    (rect, timeline, ink): (Rect, &SeekTimeline, Ink),
    live: &LiveValues,
    palette: &Palette,
) {
    let played = timeline.fraction_at(live.now);
    let y = rect.y + rect.h / 2.0;
    let head = rect.x + rect.w * played;
    let ink = palette.resolve(ink);
    if head > rect.x {
        kit.set_ink(ink);
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
    kit.station(ctx, (head, y), StationMark::Filled, ink, palette.floor_high);
}
