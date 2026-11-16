//! The island's fill and border paths, cached by shape and origin, and the floor gradient brush,
//! cached by palette. A frame at rest or in ambient rebuilds none of them.

use super::{color, f};
use plith_core::geometry::IslandShape;
use plith_core::outline::{PathSeg, island_outline};
use plith_core::theme::Palette;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_F, D2D1_FIGURE_BEGIN, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_GRADIENT_STOP,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1DeviceContext,
    ID2D1Factory1, ID2D1LinearGradientBrush, ID2D1PathGeometry1, ID2D1RenderTarget,
};
use windows::core::Result;
use windows_numerics::Vector2;

/// The island's drawing objects for one frame.
pub(super) struct Parts {
    /// The closed outline: filled, and the clip of the pages.
    pub(super) fill: ID2D1PathGeometry1,
    /// The outline inset by half a pixel without its top edge; `None` for a sliver island.
    pub(super) border: Option<ID2D1PathGeometry1>,
    /// The vertical floor gradient over the island's height.
    pub(super) gradient: ID2D1LinearGradientBrush,
}

/// The cache behind [`Parts`].
#[derive(Default)]
pub(super) struct Island {
    key: Option<[u64; 5]>,
    paths: Option<(ID2D1PathGeometry1, Option<ID2D1PathGeometry1>)>,
    gradient: Option<(ID2D1LinearGradientBrush, [[f32; 4]; 2])>,
    builds: u64,
}

fn point(x: f64, y: f64) -> Vector2 {
    Vector2 { X: f(x), Y: f(y) }
}

/// One figure starting at `start` through `segs`.
fn build_path(
    factory: &ID2D1Factory1,
    start: (f64, f64),
    segs: &[PathSeg],
    begin: D2D1_FIGURE_BEGIN,
    end: D2D1_FIGURE_END,
) -> Result<ID2D1PathGeometry1> {
    // SAFETY: the factory is live; the sink is opened once, the figure is begun and ended,
    // and the sink is closed before the geometry is used.
    unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        sink.BeginFigure(point(start.0, start.1), begin);
        for seg in segs {
            match *seg {
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
        sink.EndFigure(end);
        sink.Close()?;
        Ok(geometry)
    }
}

/// The border: the outline inset by half a pixel so a 1 DIP stroke covers the island's own
/// edge pixels, as an open figure that skips the top edge (it starts at the top-right corner).
fn border_path(
    factory: &ID2D1Factory1,
    shape: IslandShape,
    origin: (f64, f64),
) -> Result<Option<ID2D1PathGeometry1>> {
    if shape.width <= 1.0 || shape.height <= 0.5 {
        return Ok(None);
    }
    let inset = IslandShape {
        width: shape.width - 1.0,
        height: shape.height - 0.5,
        radius: (shape.radius - 0.5).max(0.0),
    };
    let start = (origin.0 + 0.5, origin.1);
    let segs = island_outline(inset, start);
    let Some((PathSeg::Line(x, y), rest)) = segs.split_first() else {
        return Ok(None);
    };
    build_path(
        factory,
        (*x, *y),
        rest,
        D2D1_FIGURE_BEGIN_HOLLOW,
        D2D1_FIGURE_END_OPEN,
    )
    .map(Some)
}

fn gradient_brush(ctx: &ID2D1DeviceContext, palette: &Palette) -> Result<ID2D1LinearGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: color(palette.floor, 1.0),
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: color(palette.floor_high, 1.0),
        },
    ];
    // The render target's version: straight sRGB stops interpolated in gamma 2.2, like CSS.
    let target: &ID2D1RenderTarget = ctx;
    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES::default();
    // SAFETY: `stops` and `props` are live for the calls.
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        target.CreateLinearGradientBrush(&raw const props, None, &collection)
    }
}

impl Island {
    /// The parts for `shape` at `origin` in `palette`: paths rebuilt only when the shape or
    /// origin changed, the gradient brush only when the floor colours changed.
    pub(super) fn update(
        &mut self,
        ctx: &ID2D1DeviceContext,
        factory: &ID2D1Factory1,
        shape: IslandShape,
        origin: (f64, f64),
        palette: &Palette,
    ) -> Result<Parts> {
        let floors = [palette.floor, palette.floor_high];
        let (top, bottom) = (point(0.0, origin.1), point(0.0, origin.1 + shape.height));
        let key = [
            shape.width.to_bits(),
            shape.height.to_bits(),
            shape.radius.to_bits(),
            origin.0.to_bits(),
            origin.1.to_bits(),
        ];
        let moved = self.key != Some(key);
        let brush = match &self.gradient {
            Some((brush, cached)) if *cached == floors => brush.clone(),
            _ => {
                let brush = gradient_brush(ctx, palette)?;
                // SAFETY: plain setters on a live brush.
                unsafe {
                    brush.SetStartPoint(top);
                    brush.SetEndPoint(bottom);
                }
                self.gradient = Some((brush.clone(), floors));
                brush
            }
        };
        let paths = match &self.paths {
            Some(paths) if !moved => paths.clone(),
            _ => {
                let fill = build_path(
                    factory,
                    origin,
                    &island_outline(shape, origin),
                    D2D1_FIGURE_BEGIN_FILLED,
                    D2D1_FIGURE_END_CLOSED,
                )?;
                let paths = (fill, border_path(factory, shape, origin)?);
                // SAFETY: plain setters on a live brush.
                unsafe {
                    brush.SetStartPoint(top);
                    brush.SetEndPoint(bottom);
                }
                self.paths = Some(paths.clone());
                self.key = Some(key);
                self.builds += 1;
                paths
            }
        };
        Ok(Parts {
            fill: paths.0,
            border: paths.1,
            gradient: brush,
        })
    }

    /// How many times the paths were built.
    pub(super) fn builds(&self) -> u64 {
        self.builds
    }
}
