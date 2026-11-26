//! The painter: the island (a floor gradient, or a material's raster floor, with a border,
//! square top, rounded bottom), the cached page bitmaps composited inside its clip, the live
//! nodes of the incoming page and the pill. Pages draw in the open palette, the pill in the
//! collapsed one.

mod island;
mod material;

use crate::list::{ListEnv, ListKit};
use crate::live::draw_live;
use crate::pages::{PILL, PageCache};
use crate::text::TextSystem;
use island::Island;
use material::Material;
use plith_core::ambient::LiveValues;
use plith_core::draw::{DisplayList, Node, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::material::art::ThemePaint;
use plith_core::material::{MaterialLayers, idle_look_alpha};
use plith_core::page::{PageLayer, PageLayers};
use plith_core::theme::Palette;
use std::sync::Arc;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LAYER_OPTIONS1_NONE,
    D2D1_LAYER_PARAMETERS1, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory1, ID2D1Geometry,
    ID2D1Layer,
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
    pub pages: PageLayers,
    /// Opacity of the pages (0 hides them); the pill is drawn at `1 - content_opacity`.
    pub content_opacity: f32,
    /// The display lists and artwork to draw.
    pub content: Arc<PageContent>,
    /// The mailbox generation `content` arrived with; it changes only when the content does.
    pub content_gen: u64,
    /// Values sampled for the live nodes on an ambient frame; the default on any other frame.
    pub live: LiveValues,
    /// The material's layers this frame; `MaterialLayers::OPEN` and ignored for a plain theme.
    pub material: MaterialLayers,
}

/// Narrows a DIP value for Direct2D.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn f(v: f64) -> f32 {
    v as f32
}

/// A straight RGBA colour with its alpha scaled by `alpha_scale`.
pub(crate) fn color(rgba: [f32; 4], alpha_scale: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: rgba[0],
        g: rgba[1],
        b: rgba[2],
        a: rgba[3] * alpha_scale,
    }
}

/// Rounds a DIP coordinate to the nearest device pixel at `scale`, so a cached bitmap (or the
/// live nodes over it) lands pixel for pixel and stays sharp at fractional scales.
pub(crate) fn snap(v: f64, scale: f64) -> f64 {
    if scale > 0.0 {
        (v * scale).round() / scale
    } else {
        v
    }
}

/// A page bitmap ready to composite: where, how opaque and at which scale.
struct Placed {
    bitmap: ID2D1Bitmap1,
    x: f64,
    y: f64,
    opacity: f32,
    scale: f64,
}

/// Draws frames onto the target set on a Direct2D device context. Render thread only.
pub struct Painter {
    factory: ID2D1Factory1,
    kit: ListKit,
    island: Island,
    pages: PageCache,
    /// The pages' colours.
    open: Palette,
    /// The pill's colours.
    collapsed: Palette,
    material: Material,
    text: TextSystem,
}

/// The context's DIP scale (its DPI over 96).
fn context_scale(ctx: &ID2D1DeviceContext) -> f64 {
    let (mut x, mut y) = (96.0_f32, 96.0_f32);
    // SAFETY: both out pointers refer to live locals.
    unsafe { ctx.GetDpi(&raw mut x, &raw mut y) };
    f64::from(x) / 96.0
}

impl Painter {
    /// Creates the painter on `ctx`; `factory` (the context's own factory) builds geometry.
    /// `text` lays out the display lists' text; `palette` colours everything (a plain theme
    /// until [`Painter::set_theme`]).
    ///
    /// # Errors
    /// Returns the Windows error when a brush cannot be created.
    pub fn new(
        ctx: &ID2D1DeviceContext,
        factory: &ID2D1Factory1,
        palette: Palette,
        text: TextSystem,
    ) -> Result<Self> {
        Ok(Self {
            factory: factory.clone(),
            kit: ListKit::new(ctx, factory)?,
            island: Island::default(),
            pages: PageCache::default(),
            open: palette,
            collapsed: palette,
            material: Material::default(),
            text,
        })
    }

    /// Switches the theme: the page and pill bitmaps are redrawn on their next use, the
    /// material's bitmaps are made on the next frame (and dropped for a plain theme).
    pub fn set_theme(&mut self, paint: ThemePaint) {
        let ThemePaint {
            open,
            collapsed,
            material,
            ..
        } = paint;
        self.open = open;
        self.collapsed = collapsed;
        self.material.set(material);
    }

    /// The palette a cache slot draws in: the pill's collapsed one, else the open one.
    fn slot_palette(&self, slot: usize) -> &Palette {
        if slot == PILL {
            &self.collapsed
        } else {
            &self.open
        }
    }

    /// How many page or pill bitmaps were redrawn so far (a test hook).
    #[must_use]
    #[doc(hidden)]
    pub fn page_rebuilds(&self) -> u64 {
        self.pages.rebuilds()
    }

    /// How many times the material's bitmaps were built so far (a test hook).
    #[must_use]
    #[doc(hidden)]
    pub fn material_builds(&self) -> u64 {
        self.material.builds()
    }

    /// How many times the island geometry was built so far (a test hook).
    #[must_use]
    #[doc(hidden)]
    pub fn island_builds(&self) -> u64 {
        self.island.builds()
    }

    /// Re-rasters the bitmaps of `pages` (page indices into `content`) whose list changed since
    /// their last raster. Outside any frame's `BeginDraw`.
    ///
    /// # Errors
    /// Returns the failing bitmap creation, draw or `EndDraw` call.
    pub fn prewarm(
        &mut self,
        ctx: &ID2D1DeviceContext,
        content: &PageContent,
        pages: &[usize],
    ) -> Result<()> {
        let scale = context_scale(ctx);
        for &page in pages {
            if let Some(list) = content.pages.get(page) {
                let palette = *self.slot_palette(page);
                let mut env = ListEnv {
                    palette: &palette,
                    text: &mut self.text,
                    art: &content.art,
                    scale,
                };
                self.pages.draw(ctx, page, list, &mut self.kit, &mut env)?;
            }
        }
        Ok(())
    }

    /// Drains the text system's notes (font fallbacks), for the log.
    pub fn take_notes(&mut self) -> Vec<String> {
        self.text.take_notes()
    }

    /// Clears the target and draws `frame`. The context must already have a target bitmap.
    /// The page and pill bitmaps are refreshed first, outside this frame's `BeginDraw`.
    ///
    /// # Errors
    /// Returns the `EndDraw` error (for example device removed) or a resource failure.
    pub fn paint(&mut self, ctx: &ID2D1DeviceContext, frame: &PaintFrame) -> Result<()> {
        let scale = context_scale(ctx);
        let parts =
            self.island
                .update(ctx, &self.factory, frame.shape, frame.origin, &self.open)?;
        let placed = self.place_pages(ctx, frame, scale)?;
        self.material.ready(ctx)?;
        let material = self.material.is_set();
        // A material never shows through: an opaque base in the collapsed floor colour lies
        // under the pill and the reveal, skipped once the opaque fabric covers the shape.
        let base = material && frame.material.reveal < 1.0;
        if base {
            self.kit.set_color(self.collapsed.floor, 1.0);
        }
        let clear = color([0.0; 4], 1.0);
        // SAFETY: draw calls between BeginDraw and EndDraw on a live context with a target.
        unsafe {
            ctx.BeginDraw();
            ctx.Clear(Some(&raw const clear));
            if !material {
                ctx.FillGeometry(&parts.fill, &parts.gradient, None);
            } else if base {
                ctx.FillGeometry(&parts.fill, &self.kit.brush, None);
            }
        }
        let clipped = material || !placed.is_empty();
        if clipped {
            push_clip(ctx, parts.fill.clone().into());
        }
        if material {
            let (shape, origin) = (frame.shape, frame.origin);
            self.material
                .draw(ctx, shape, origin, &frame.material, scale);
        }
        for page in &placed {
            composite(ctx, page);
        }
        self.draw_layers_live(ctx, frame, scale);
        if clipped {
            // SAFETY: as above; pairs the PushLayer of `push_clip`.
            unsafe { ctx.PopLayer() };
        }
        if let Some(border) = &parts.border {
            // A material's border follows its floor: the pill's while the pill shows.
            let pill = if material {
                frame.material.pill.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let strokes = [
                (self.collapsed.border, pill),
                (self.open.border, 1.0 - pill),
            ];
            for (rgba, alpha) in strokes {
                if alpha > 0.0 {
                    self.kit.set_color(rgba, alpha);
                    // SAFETY: as above.
                    unsafe { ctx.DrawGeometry(border, &self.kit.brush, 1.0, None) };
                }
            }
        }
        // SAFETY: closes the BeginDraw above.
        unsafe { ctx.EndDraw(None, None) }
    }

    /// The idle look's opacity this frame: `1 - content_opacity`, and with a material never
    /// above its pill, so the look leaves with the black pill (`idle_look_alpha`).
    fn idle_alpha(&self, frame: &PaintFrame) -> f32 {
        let pill = self.material.is_set().then_some(frame.material.pill);
        idle_look_alpha(frame.content_opacity, pill)
    }

    /// Refreshes the bitmaps this frame shows and places them: the visible page layers (when
    /// the content is visible), then the idle look's pill list (at `idle_alpha`).
    fn place_pages(
        &mut self,
        ctx: &ID2D1DeviceContext,
        frame: &PaintFrame,
        scale: f64,
    ) -> Result<Vec<Placed>> {
        let (ox, oy) = frame.origin;
        let content = &frame.content;
        let (open, collapsed) = (self.open, self.collapsed);
        let idle = self.idle_alpha(frame);
        let mut env = ListEnv {
            palette: &open,
            text: &mut self.text,
            art: &content.art,
            scale,
        };
        let mut placed = Vec::with_capacity(3);
        let mut add = |slot: usize, list: &Arc<DisplayList>, x: f64, opacity: f32| -> Result<()> {
            if opacity <= 0.0 {
                return Ok(());
            }
            env.palette = if slot == PILL { &collapsed } else { &open };
            let bitmap = self.pages.draw(ctx, slot, list, &mut self.kit, &mut env)?;
            if let Some(bitmap) = bitmap {
                placed.push(Placed {
                    bitmap,
                    x,
                    y: oy,
                    opacity,
                    scale,
                });
            }
            Ok(())
        };
        if frame.content_opacity > 0.0 {
            for layer in frame.pages.iter() {
                if let Some(list) = content.pages.get(layer.page) {
                    let opacity = f(layer.opacity) * frame.content_opacity;
                    add(layer.page, list, ox + layer.offset, opacity)?;
                }
            }
        }
        add(PILL, &content.pill, ox, idle)?;
        Ok(placed)
    }

    /// Draws the live nodes of every visible page layer (outgoing first) at its offset and
    /// opacity, then the pill's (the title scroll) at `idle_alpha`.
    fn draw_layers_live(&mut self, ctx: &ID2D1DeviceContext, frame: &PaintFrame, scale: f64) {
        let (ox, oy) = frame.origin;
        for layer in frame.pages.iter() {
            let PageLayer {
                page,
                offset,
                opacity,
            } = *layer;
            if let Some(list) = frame.content.pages.get(page) {
                let alpha = f(opacity) * frame.content_opacity;
                let at = (page, ox + offset, oy);
                self.draw_list_live(ctx, frame, (list, alpha), at, scale);
            }
        }
        let pill = &frame.content.pill;
        let alpha = self.idle_alpha(frame);
        self.draw_list_live(ctx, frame, (pill, alpha), (PILL, ox, oy), scale);
    }

    /// Draws the live nodes of `list` (cache slot `slot`, which picks the palette) at `alpha`,
    /// offset to the same snapped place as its bitmap at `(x, y)`. Nothing for a hidden list or
    /// one without live nodes.
    fn draw_list_live(
        &mut self,
        ctx: &ID2D1DeviceContext,
        frame: &PaintFrame,
        (list, alpha): (&DisplayList, f32),
        (slot, x, y): (usize, f64, f64),
        scale: f64,
    ) {
        if alpha <= 0.0 || !list.nodes.iter().any(|node| matches!(node, Node::Live(_))) {
            return;
        }
        let shift = Matrix3x2::translation(f(snap(x, scale)), f(snap(y, scale)));
        // SAFETY: plain state changes inside the frame's BeginDraw/EndDraw; restored below.
        unsafe { ctx.SetTransform(&raw const shift) };
        let palette = *self.slot_palette(slot);
        draw_live(
            ctx,
            (&mut self.kit, &mut self.text),
            list,
            &frame.live,
            &palette,
            alpha,
        );
        let identity = Matrix3x2::identity();
        // SAFETY: as above.
        unsafe { ctx.SetTransform(&raw const identity) };
    }
}

/// Draws a cached bitmap at its place with its group opacity, at its own DIP size.
fn composite(ctx: &ID2D1DeviceContext, page: &Placed) {
    // SAFETY: plain draw calls inside the frame's BeginDraw/EndDraw on a live bitmap.
    unsafe {
        let size = page.bitmap.GetSize();
        let (x, y) = (f(snap(page.x, page.scale)), f(snap(page.y, page.scale)));
        let dest = D2D_RECT_F {
            left: x,
            top: y,
            right: x + size.width,
            bottom: y + size.height,
        };
        ctx.DrawBitmap(
            &page.bitmap,
            Some(&raw const dest),
            page.opacity,
            D2D1_INTERPOLATION_MODE_LINEAR,
            None,
            None,
        );
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
