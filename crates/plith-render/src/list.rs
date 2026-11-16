//! Draws the cached nodes of a display list: fills, strokes, stations, text and artwork. Live
//! nodes are skipped here; they are drawn every frame by `live`.

use crate::paint::{color, f};
use crate::text::TextSystem;
use plith_core::draw::{
    Align, ArtId, ArtPixels, DisplayList, Node, Rect, StationMark, TextStyle, Trim,
};
use plith_core::theme::Palette;
use std::collections::HashMap;
use std::sync::Arc;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_CUSTOM,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LINE_JOIN_MITER,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, ID2D1Bitmap1, ID2D1DeviceContext,
    ID2D1Factory, ID2D1Factory1, ID2D1PathGeometry1, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;
use windows_numerics::Vector2;

/// The dash and the gap of a dashed stroke, in DIP (the mock's `stroke-dasharray: 3 4`).
const DASH: (f32, f32) = (3.0, 4.0);
/// A station's outer side in DIP: the mock's 7 DIP square with a centred 1 DIP stroke.
pub(crate) const STATION: f64 = 8.0;
/// The most artwork bitmaps kept before the cache is cleared.
const ART_CAP: usize = 64;

/// What a list is drawn with besides the device objects.
pub(crate) struct ListEnv<'a> {
    /// The colours.
    pub(crate) palette: &'a Palette,
    /// Text layouts.
    pub(crate) text: &'a mut TextSystem,
    /// The artwork `Node::Image` refers to.
    pub(crate) art: &'a [Arc<ArtPixels>],
    /// The context's DIP scale, which the cached bitmaps are drawn at.
    pub(crate) scale: f64,
}

/// The device objects list drawing reuses: one brush, the dash styles, artwork bitmaps.
pub(crate) struct ListKit {
    /// The one solid brush; its colour is set per node.
    pub(crate) brush: ID2D1SolidColorBrush,
    factory: ID2D1Factory1,
    /// Dash styles by stroke width bits: Direct2D dashes scale with the width, so each width
    /// gets the dash and gap divided by it.
    dashes: HashMap<u32, ID2D1StrokeStyle>,
    art: HashMap<ArtId, ID2D1Bitmap1>,
    /// Scales every colour's alpha (the live nodes' group opacity); 1 in the cache pass.
    alpha: f32,
}

fn rect_f(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: f(r.x),
        top: f(r.y),
        right: f(r.x + r.w),
        bottom: f(r.y + r.h),
    }
}

/// An open polyline through `points`; `None` for fewer than two points.
pub(crate) fn polyline(
    factory: &ID2D1Factory1,
    points: impl IntoIterator<Item = (f64, f64)>,
) -> Result<Option<ID2D1PathGeometry1>> {
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return Ok(None);
    };
    let mut count = 1;
    // SAFETY: the factory is live; the sink is opened once, the figure begun and ended, and the
    // sink closed before the geometry is used.
    unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        sink.BeginFigure(
            Vector2 {
                X: f(first.0),
                Y: f(first.1),
            },
            D2D1_FIGURE_BEGIN_HOLLOW,
        );
        for (x, y) in points {
            sink.AddLine(Vector2 { X: f(x), Y: f(y) });
            count += 1;
        }
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.Close()?;
        Ok((count > 1).then_some(geometry))
    }
}

impl ListKit {
    /// Creates the brush on `ctx`; `factory` builds paths and stroke styles.
    pub(crate) fn new(ctx: &ID2D1DeviceContext, factory: &ID2D1Factory1) -> Result<Self> {
        let initial = color([0.0, 0.0, 0.0, 1.0], 1.0);
        // SAFETY: `initial` is live for the call.
        let brush = unsafe { ctx.CreateSolidColorBrush(&raw const initial, None)? };
        Ok(Self {
            brush,
            factory: factory.clone(),
            dashes: HashMap::new(),
            art: HashMap::new(),
            alpha: 1.0,
        })
    }

    /// Sets the brush to `rgba` with its alpha scaled by `alpha` and remembers the scale for
    /// the following `set_ink` calls.
    pub(crate) fn set_color(&mut self, rgba: [f32; 4], alpha: f32) {
        self.alpha = alpha;
        self.set_ink(rgba);
    }

    /// Sets the brush to `rgba` at the current alpha scale.
    pub(crate) fn set_ink(&self, rgba: [f32; 4]) {
        let c = color(rgba, self.alpha);
        // SAFETY: plain setter on a live brush.
        unsafe { self.brush.SetColor(&raw const c) };
    }

    /// A filled rounded rectangle.
    pub(crate) fn fill(&self, ctx: &ID2D1DeviceContext, rect: Rect, radius: f64, rgba: [f32; 4]) {
        let r = radius.min(rect.w / 2.0).min(rect.h / 2.0).max(0.0);
        let rounded = D2D1_ROUNDED_RECT {
            rect: rect_f(rect),
            radiusX: f(r),
            radiusY: f(r),
        };
        self.set_ink(rgba);
        // SAFETY: a draw call inside a BeginDraw/EndDraw on a live context.
        unsafe { ctx.FillRoundedRectangle(&raw const rounded, &self.brush) };
    }

    /// The dash style for `width`, created once per width.
    fn dash_style(&mut self, width: f32) -> Result<ID2D1StrokeStyle> {
        if let Some(style) = self.dashes.get(&width.to_bits()) {
            return Ok(style.clone());
        }
        let props = D2D1_STROKE_STYLE_PROPERTIES {
            startCap: D2D1_CAP_STYLE_FLAT,
            endCap: D2D1_CAP_STYLE_FLAT,
            dashCap: D2D1_CAP_STYLE_FLAT,
            lineJoin: D2D1_LINE_JOIN_MITER,
            miterLimit: 10.0,
            dashStyle: D2D1_DASH_STYLE_CUSTOM,
            dashOffset: 0.0,
        };
        let unit = width.max(0.01);
        let dashes = [DASH.0 / unit, DASH.1 / unit];
        let factory: &ID2D1Factory = &self.factory;
        // SAFETY: `props` and `dashes` are live for the call.
        let style = unsafe { factory.CreateStrokeStyle(&raw const props, Some(&dashes))? };
        self.dashes.insert(width.to_bits(), style.clone());
        Ok(style)
    }

    /// A polyline of `width`, dashed (dash 4, gap 3 DIP) when asked.
    pub(crate) fn stroke(
        &mut self,
        ctx: &ID2D1DeviceContext,
        points: &[(f64, f64)],
        width: f64,
        rgba: [f32; 4],
        dashed: bool,
    ) -> Result<()> {
        let Some(path) = polyline(&self.factory, points.iter().copied())? else {
            return Ok(());
        };
        let width = f(width);
        let style = if dashed {
            Some(self.dash_style(width)?)
        } else {
            None
        };
        self.set_ink(rgba);
        // SAFETY: a draw call inside a BeginDraw/EndDraw on a live context.
        unsafe { ctx.DrawGeometry(&path, &self.brush, width, style.as_ref()) };
        Ok(())
    }

    /// An 8 DIP station square centred at (x, y): filled with `rgba`, or a 1 DIP inner ring of
    /// `rgba` over `floor` (the floor colour at that height) that hides a line running under it.
    pub(crate) fn station(
        &self,
        ctx: &ID2D1DeviceContext,
        at: (f64, f64),
        mark: StationMark,
        rgba: [f32; 4],
        floor: [f32; 4],
    ) {
        let half = STATION / 2.0;
        let square = rect_f(Rect {
            x: at.0 - half,
            y: at.1 - half,
            w: STATION,
            h: STATION,
        });
        // SAFETY: draw calls inside a BeginDraw/EndDraw on a live context.
        unsafe {
            if mark == StationMark::Filled {
                self.set_ink(rgba);
                ctx.FillRectangle(&raw const square, &self.brush);
                return;
            }
            self.set_ink(floor);
            ctx.FillRectangle(&raw const square, &self.brush);
            let ring = rect_f(Rect {
                x: at.0 - half + 0.5,
                y: at.1 - half + 0.5,
                w: STATION - 1.0,
                h: STATION - 1.0,
            });
            self.set_ink(rgba);
            ctx.DrawRectangle(&raw const ring, &self.brush, 1.0, None);
        }
    }

    /// A filled polygon through `points` (closed implicitly); fewer than three points draw
    /// nothing.
    pub(crate) fn polygon(
        &self,
        ctx: &ID2D1DeviceContext,
        points: &[(f64, f64)],
        rgba: [f32; 4],
    ) -> Result<()> {
        if points.len() < 3 {
            return Ok(());
        }
        // SAFETY: the factory is live; the sink is opened once, the figure begun and ended, and
        // the sink closed before the geometry is drawn inside the open BeginDraw.
        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            let at = |p: (f64, f64)| Vector2 {
                X: f(p.0),
                Y: f(p.1),
            };
            sink.BeginFigure(at(points[0]), D2D1_FIGURE_BEGIN_FILLED);
            for p in &points[1..] {
                sink.AddLine(at(*p));
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.set_ink(rgba);
            ctx.FillGeometry(&geometry, &self.brush, None);
        }
        Ok(())
    }

    /// One line of text laid out to the rectangle's width and aligned in it.
    pub(crate) fn text(
        &self,
        ctx: &ID2D1DeviceContext,
        text: &mut TextSystem,
        node: (Rect, &str, TextStyle, Align, Trim),
        rgba: [f32; 4],
    ) -> Result<()> {
        let (rect, content, style, align, trim) = node;
        if rect.w <= 0.0 {
            return Ok(());
        }
        let layout = text.layout(content, style, f(rect.w), trim)?;
        let alignment = match align {
            Align::Start => DWRITE_TEXT_ALIGNMENT_LEADING,
            Align::End => DWRITE_TEXT_ALIGNMENT_TRAILING,
            Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
        };
        self.set_ink(rgba);
        let origin = Vector2 {
            X: f(rect.x),
            Y: f(rect.y),
        };
        // SAFETY: the layout is live; the draw call is inside a BeginDraw/EndDraw.
        unsafe {
            layout.SetTextAlignment(alignment)?;
            ctx.DrawTextLayout(origin, &layout, &self.brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
        }
        Ok(())
    }

    /// The bitmap of `id`, created from its pixels on first use; `None` when the artwork is
    /// missing or its pixels are malformed.
    fn art_bitmap(
        &mut self,
        ctx: &ID2D1DeviceContext,
        id: ArtId,
        art: &[Arc<ArtPixels>],
    ) -> Result<Option<ID2D1Bitmap1>> {
        if let Some(bitmap) = self.art.get(&id) {
            return Ok(Some(bitmap.clone()));
        }
        let Some(pixels) = art.iter().find(|a| a.id == id) else {
            return Ok(None);
        };
        let pitch = pixels.width.saturating_mul(4);
        let needed = u64::from(pitch) * u64::from(pixels.height);
        if pixels.width == 0 || pixels.height == 0 || (pixels.bgra.len() as u64) < needed {
            return Ok(None);
        }
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        let size = D2D_SIZE_U {
            width: pixels.width,
            height: pixels.height,
        };
        // SAFETY: the pixel buffer holds at least `pitch * height` bytes (checked above) and
        // outlives the call, which copies it; `props` is live.
        let bitmap = unsafe {
            ctx.CreateBitmap(
                size,
                Some(pixels.bgra.as_ptr().cast()),
                pitch,
                &raw const props,
            )?
        };
        if self.art.len() >= ART_CAP {
            self.art.clear();
        }
        self.art.insert(id, bitmap.clone());
        Ok(Some(bitmap))
    }

    /// Artwork scaled into `rect`; false when the artwork is missing.
    fn image(
        &mut self,
        ctx: &ID2D1DeviceContext,
        rect: Rect,
        id: ArtId,
        art: &[Arc<ArtPixels>],
    ) -> Result<bool> {
        let Some(bitmap) = self.art_bitmap(ctx, id, art)? else {
            return Ok(false);
        };
        let dest = rect_f(rect);
        // SAFETY: a draw call inside a BeginDraw/EndDraw on a live bitmap.
        unsafe {
            ctx.DrawBitmap(
                &bitmap,
                Some(&raw const dest),
                self.alpha,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            );
        }
        Ok(true)
    }
}

/// The island's floor gradient at `t` (0 at the top, 1 at the bottom), mixed in sRGB like
/// the gradient brush.
pub(crate) fn floor_at(palette: &Palette, t: f64) -> [f32; 4] {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let (a, b) = (palette.floor, palette.floor_high);
    let mix = |i: usize| a[i] + (b[i] - a[i]) * f(t);
    [mix(0), mix(1), mix(2), 1.0]
}

/// Draws the nodes of `list` (live nodes skipped) inside an open `BeginDraw`, at full alpha.
/// Returns the artwork that was referenced but missing, so the caller can redraw once it comes.
///
/// # Errors
/// Returns the first failing geometry, text or bitmap call.
pub(crate) fn draw_list(
    ctx: &ID2D1DeviceContext,
    kit: &mut ListKit,
    env: &mut ListEnv<'_>,
    list: &DisplayList,
) -> Result<Vec<ArtId>> {
    kit.alpha = 1.0;
    let palette = env.palette;
    let mut missing = Vec::new();
    for node in &list.nodes {
        match node {
            Node::Fill { rect, radius, ink } => {
                kit.fill(ctx, *rect, *radius, palette.resolve(*ink));
            }
            Node::Stroke {
                points,
                width,
                ink,
                dashed,
            } => kit.stroke(ctx, points, *width, palette.resolve(*ink), *dashed)?,
            Node::Station { x, y, mark, ink } => kit.station(
                ctx,
                (*x, *y),
                *mark,
                palette.resolve(*ink),
                floor_at(palette, *y / list.size.1),
            ),
            Node::Polygon { points, ink } => kit.polygon(ctx, points, palette.resolve(*ink))?,
            Node::Text {
                rect,
                text,
                style,
                ink,
                align,
                trim,
            } => kit.text(
                ctx,
                env.text,
                (*rect, text, *style, *align, *trim),
                palette.resolve(*ink),
            )?,
            Node::Image { rect, art } => {
                if !kit.image(ctx, *rect, *art, env.art)? {
                    missing.push(*art);
                }
            }
            Node::Live(_) => {}
        }
    }
    Ok(missing)
}
