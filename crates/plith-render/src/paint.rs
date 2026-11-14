//! The painter: a black rounded island and one placeholder bar per visible page layer.

use plith_core::geometry::IslandShape;
use plith_core::page::PageLayer;
use plith_core::shapes::{FILL, PLACEHOLDER};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1,
    D2D1_ROUNDED_RECT, ID2D1DeviceContext, ID2D1Geometry, ID2D1Layer, ID2D1SolidColorBrush,
};
use windows::core::Result;
use windows_numerics::Matrix3x2;

/// Everything the painter needs for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintFrame {
    /// Current island size and corner radius in DIP.
    pub shape: IslandShape,
    /// Island top-left in DIP within the surface.
    pub origin: (f64, f64),
    /// Visible page layers, outgoing first.
    pub pages: Vec<PageLayer>,
    /// Clip the placeholders to the island while the shape is unsettled.
    pub clip: bool,
}

/// Draws frames onto the target set on a Direct2D device context.
pub struct Painter {
    brush: ID2D1SolidColorBrush,
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

impl Painter {
    /// Creates the painter's one solid colour brush on `ctx`.
    ///
    /// # Errors
    /// Returns the Windows error when the brush cannot be created.
    pub fn new(ctx: &ID2D1DeviceContext) -> Result<Self> {
        let initial = color(FILL, 1.0);
        // SAFETY: `initial` is live for the call.
        let brush = unsafe { ctx.CreateSolidColorBrush(&raw const initial, None)? };
        Ok(Self { brush })
    }

    /// Clears the target and draws `frame`. The context must already have a target bitmap.
    ///
    /// # Errors
    /// Returns the `EndDraw` error (for example device removed) or a geometry failure.
    pub fn paint(&mut self, ctx: &ID2D1DeviceContext, frame: &PaintFrame) -> Result<()> {
        let (ox, oy) = frame.origin;
        let shape = frame.shape;
        let island = rounded(ox, oy, shape.width, shape.height, shape.radius);
        let clear = color([0.0; 4], 1.0);
        // Built before BeginDraw so a failure here cannot leave the context mid-draw.
        let clip = if frame.clip {
            Some(Self::clip_geometry(ctx, &island)?)
        } else {
            None
        };
        // SAFETY: draw calls between BeginDraw and EndDraw on a live context with a target.
        unsafe {
            ctx.BeginDraw();
            ctx.Clear(Some(&raw const clear));
            let fill = color(FILL, 1.0);
            self.brush.SetColor(&raw const fill);
            ctx.FillRoundedRectangle(&raw const island, &self.brush);
        }
        if let Some(geometry) = clip.clone() {
            Self::push_clip(ctx, geometry);
        }
        for layer in &frame.pages {
            let x = ox + (shape.width - PLACEHOLDER.width) / 2.0 + layer.offset;
            let bar = rounded(
                x,
                oy + PLACEHOLDER.top,
                PLACEHOLDER.width,
                PLACEHOLDER.height,
                PLACEHOLDER.radius,
            );
            let tint = color(PLACEHOLDER.rgba, f(layer.opacity));
            // SAFETY: as above.
            unsafe {
                self.brush.SetColor(&raw const tint);
                ctx.FillRoundedRectangle(&raw const bar, &self.brush);
            }
        }
        // SAFETY: as above; PopLayer pairs the PushLayer of `push_clip`.
        unsafe {
            if clip.is_some() {
                ctx.PopLayer();
            }
            ctx.EndDraw(None, None)
        }
    }

    /// Builds the island's rounded-rect geometry.
    fn clip_geometry(
        ctx: &ID2D1DeviceContext,
        island: &D2D1_ROUNDED_RECT,
    ) -> Result<ID2D1Geometry> {
        // SAFETY: the factory is live and `island` is valid for the call.
        unsafe {
            let factory = ctx.GetFactory()?;
            Ok(factory.CreateRoundedRectangleGeometry(island)?.into())
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
