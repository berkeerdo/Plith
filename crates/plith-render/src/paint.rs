//! The painter: a black island (square top, rounded bottom) and one placeholder bar per visible page layer.

use plith_core::ambient::LiveValues;
use plith_core::draw::PageContent;
use plith_core::geometry::IslandShape;
use plith_core::outline::{PathSeg, island_outline};
use plith_core::page::PageLayers;
use plith_core::shapes::{FILL, PLACEHOLDER};
use std::sync::Arc;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL,
    D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1, D2D1_ROUNDED_RECT,
    D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1DeviceContext, ID2D1Factory1, ID2D1Geometry, ID2D1Layer,
    ID2D1PathGeometry1, ID2D1SolidColorBrush,
};
use windows::core::Result;
use windows_numerics::{Matrix3x2, Vector2};

/// Everything the painter needs for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintFrame {
    /// Current island size and corner radius in DIP.
    pub shape: IslandShape,
    /// Island top-left in DIP within the surface.
    pub origin: (f64, f64),
    /// Visible page layers, outgoing first.
    pub pages: PageLayers,
    /// Opacity of the page placeholders (0 hides them); the island clips them always.
    pub content_opacity: f32,
    /// The display lists and artwork to draw.
    pub content: Arc<PageContent>,
    /// The mailbox generation `content` arrived with; it changes only when the content does.
    pub content_gen: u64,
    /// Values sampled for the live nodes on an ambient frame; the default on any other frame.
    pub live: LiveValues,
}

/// Draws frames onto the target set on a Direct2D device context.
pub struct Painter {
    brush: ID2D1SolidColorBrush,
    factory: ID2D1Factory1,
}

#[allow(clippy::cast_possible_truncation)]
fn f(v: f64) -> f32 {
    v as f32
}

fn color(rgba: [f32; 4], alpha_scale: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: rgba[0],
        g: rgba[1],
        b: rgba[2],
        a: rgba[3] * alpha_scale,
    }
}

fn rounded(x: f64, y: f64, width: f64, height: f64, radius: f64) -> D2D1_ROUNDED_RECT {
    let r = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: f(x),
            top: f(y),
            right: f(x + width),
            bottom: f(y + height),
        },
        radiusX: f(r),
        radiusY: f(r),
    }
}

/// True when some placeholder bar could reach a rounded corner or leave the island, so the
/// pages must be clipped. Bars inside the straight band (above the bottom corner arcs, within
/// the width) need no clip: the top corners are square.
fn needs_clip(frame: &PaintFrame) -> bool {
    let shape = frame.shape;
    let radius = shape
        .radius
        .min(shape.width / 2.0)
        .min(shape.height / 2.0)
        .max(0.0);
    let vertical_ok = PLACEHOLDER.top + PLACEHOLDER.height <= shape.height - radius;
    let left = (shape.width - PLACEHOLDER.width) / 2.0;
    let horizontal_ok = frame.pages.iter().all(|layer| {
        let x = left + layer.offset;
        x >= 0.0 && x + PLACEHOLDER.width <= shape.width
    });
    !(vertical_ok && horizontal_ok)
}

impl Painter {
    /// Creates the painter's one solid colour brush on `ctx`; `factory` (the context's own
    /// factory) builds the clip geometry.
    ///
    /// # Errors
    /// Returns the Windows error when the brush cannot be created.
    pub fn new(ctx: &ID2D1DeviceContext, factory: &ID2D1Factory1) -> Result<Self> {
        let initial = color(FILL, 1.0);
        // SAFETY: `initial` is live for the call.
        let brush = unsafe { ctx.CreateSolidColorBrush(&raw const initial, None)? };
        Ok(Self {
            brush,
            factory: factory.clone(),
        })
    }

    /// Clears the target and draws `frame`. The context must already have a target bitmap.
    ///
    /// # Errors
    /// Returns the `EndDraw` error (for example device removed) or a geometry failure.
    pub fn paint(&mut self, ctx: &ID2D1DeviceContext, frame: &PaintFrame) -> Result<()> {
        let clear = color([0.0; 4], 1.0);
        // Built before BeginDraw so a failure here cannot leave the context mid-draw. One
        // geometry per frame serves both the fill and the page clip.
        let island = self.island_geometry(frame)?;
        let clipped = frame.content_opacity > 0.0 && !frame.pages.is_empty() && needs_clip(frame);
        // SAFETY: draw calls between BeginDraw and EndDraw on a live context with a target.
        unsafe {
            ctx.BeginDraw();
            ctx.Clear(Some(&raw const clear));
            let fill = color(FILL, 1.0);
            self.brush.SetColor(&raw const fill);
            ctx.FillGeometry(&island, &self.brush, None);
        }
        if clipped {
            Self::push_clip(ctx, island.into());
        }
        if frame.content_opacity > 0.0 {
            self.draw_pages(ctx, frame);
        }
        // SAFETY: as above; PopLayer pairs the PushLayer of `push_clip`.
        unsafe {
            if clipped {
                ctx.PopLayer();
            }
            ctx.EndDraw(None, None)
        }
    }

    /// Draws one placeholder bar per page layer, faded by the content opacity.
    fn draw_pages(&self, ctx: &ID2D1DeviceContext, frame: &PaintFrame) {
        let (ox, oy) = frame.origin;
        let shape = frame.shape;
        for layer in frame.pages.iter() {
            let x = ox + (shape.width - PLACEHOLDER.width) / 2.0 + layer.offset;
            let bar = rounded(
                x,
                oy + PLACEHOLDER.top,
                PLACEHOLDER.width,
                PLACEHOLDER.height,
                PLACEHOLDER.radius,
            );
            let tint = color(PLACEHOLDER.rgba, f(layer.opacity) * frame.content_opacity);
            // SAFETY: draw calls inside the BeginDraw/EndDraw of `paint` on a live context.
            unsafe {
                self.brush.SetColor(&raw const tint);
                ctx.FillRoundedRectangle(&raw const bar, &self.brush);
            }
        }
    }

    /// Builds the island's path geometry from the shared outline: square top, rounded bottom.
    fn island_geometry(&self, frame: &PaintFrame) -> Result<ID2D1PathGeometry1> {
        let (ox, oy) = frame.origin;
        let point = |x: f64, y: f64| Vector2 { X: f(x), Y: f(y) };
        // SAFETY: the factory is live; the sink is opened once, every figure is begun and ended,
        // and the sink is closed before the geometry is used.
        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(point(ox, oy), D2D1_FIGURE_BEGIN_FILLED);
            for seg in island_outline(frame.shape, frame.origin) {
                match seg {
                    PathSeg::Line(x, y) => sink.AddLine(point(x, y)),
                    PathSeg::Arc { to, radius } => {
                        let arc = D2D1_ARC_SEGMENT {
                            point: point(to.0, to.1),
                            size: D2D_SIZE_F {
                                width: f(radius),
                                height: f(radius),
                            },
                            rotationAngle: 0.0,
                            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                            arcSize: D2D1_ARC_SIZE_SMALL,
                        };
                        sink.AddArc(&raw const arc);
                    }
                }
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            Ok(geometry)
        }
    }

    /// Pushes a layer clipped to `geometry`.
    fn push_clip(ctx: &ID2D1DeviceContext, geometry: ID2D1Geometry) {
        // SAFETY: the geometry is moved into the layer parameters and released by the explicit
        // drop below, after PushLayer took its own reference.
        unsafe {
            let params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: D2D_RECT_F {
                    left: f32::NEG_INFINITY,
                    top: f32::NEG_INFINITY,
                    right: f32::INFINITY,
                    bottom: f32::INFINITY,
                },
                geometricMask: std::mem::ManuallyDrop::new(Some(geometry)),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: Matrix3x2::identity(),
                opacity: 1.0,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            };
            ctx.PushLayer(&raw const params, None::<&ID2D1Layer>);
            drop(std::mem::ManuallyDrop::into_inner(params.geometricMask));
        }
    }
}
