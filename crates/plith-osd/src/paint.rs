//! Draws one OSD frame: the seal (square top, round bottom), the label, the ruler and the value,
//! in three fixed slots that never reflow. Every length is in DIP; the target's DPI scales it.
//! Text comes from the shared [`TextSystem`], the same fonts and styles the pages use.

use crate::model::OsdFrame;
use plith_core::draw::{Family, TextStyle, Trim};
use plith_core::pages::copy;
use plith_ipc::Meter;
use plith_render::{Gpu, TextSystem};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TEXT_METRICS,
};
use windows::core::Result;
use windows_numerics::Vector2;

/// OSD width in DIP.
pub const WIDTH: f32 = 300.0;
/// OSD height in DIP.
pub const HEIGHT: f32 = 40.0;
/// Bottom corner radius in DIP.
const RADIUS: f32 = 16.0;
/// Side padding in DIP.
const PAD: f32 = 16.0;
/// Left slot (label) right edge.
const LABEL_RIGHT: f32 = 84.0;
/// Ruler left and right edges.
const RULER: (f32, f32) = (92.0, 228.0);
/// Right slot (value) left edge: 48 DIP wide, room for the muted word in Martian Mono
/// (45.6 DIP measured).
const VALUE_LEFT: f32 = 236.0;
/// Tick length below the track.
const TICK: f32 = 3.0;

/// The Gece palette as `0xRRGGBB`.
const FLOOR: u32 = 0x000B_0B0A;
const BORDER: u32 = 0x002B_2824;
const INK: u32 = 0x00EC_E9E2;
const MUTED: u32 = 0x00A3_9E94;
const FAINT: u32 = 0x008A_847A;
const TRACK: u32 = 0x004A_463F;
const LIME: u32 = 0x00CA_FF33;

/// The text of the right slot: the level as a whole percent, or the mute word.
#[must_use]
pub fn value_text(frame: &OsdFrame) -> String {
    if frame.muted {
        return copy::MUTED_LABEL.to_owned();
    }
    // Allowed: the level is clamped to 0..=1, so the percent fits a u8 exactly.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (frame.level.clamp(0.0, 1.0) * 100.0).round() as u8;
    percent.to_string()
}

/// The x of tick `i` (0 to 10) on the ruler.
#[must_use]
pub fn tick_x(i: u8) -> f32 {
    RULER.0 + (RULER.1 - RULER.0) * f32::from(i) / 10.0
}

/// The right edge of the ruler fill at `level` (0 to 1).
#[must_use]
pub fn fill_right(level: f32) -> f32 {
    RULER.0 + (RULER.1 - RULER.0) * level.clamp(0.0, 1.0)
}

/// `dip` moved to the nearest device pixel edge at `scale` pixels per DIP.
#[must_use]
pub fn snap(dip: f32, scale: f32) -> f32 {
    (dip * scale).round() / scale
}

/// A span from `start` of `len` DIP with both edges on device pixels and at least one device
/// pixel wide, as `(start, end)`.
#[must_use]
pub fn snap_span(start: f32, len: f32, scale: f32) -> (f32, f32) {
    let a = snap(start, scale);
    let b = snap(start + len, scale);
    if (b - a) * scale < 1.0 - 1e-4 {
        (a, a + 1.0 / scale)
    } else {
        (a, b)
    }
}

/// The thinnest line in device pixels at `scale`: one DIP rounded, never under one pixel.
#[must_use]
pub fn line_px(scale: f32) -> f32 {
    scale.round().max(1.0)
}

/// Tick `i` (0 to 10) as `(left, right)` in DIP: its centre snapped, every tick the same
/// [`line_px`] wide, both edges on device pixels.
#[must_use]
pub fn tick_span(i: u8, scale: f32) -> (f32, f32) {
    let width = line_px(scale) / scale;
    let left = snap(tick_x(i) - width / 2.0, scale);
    (left, left + width)
}

/// The track and fill rows as `((top, bottom), (top, bottom))` in DIP, centred on the
/// OSD's middle: the track is [`line_px`] thick, the fill twice that, edges on device pixels.
#[must_use]
pub fn ruler_rows(scale: f32) -> ((f32, f32), (f32, f32)) {
    let mid = HEIGHT / 2.0;
    let row = |px: f32| {
        let thick = px / scale;
        let top = snap(mid - thick / 2.0, scale);
        (top, top + thick)
    };
    (row(line_px(scale)), row(2.0 * line_px(scale)))
}

/// The top of a text line of `height` DIP centred in the OSD's height.
#[must_use]
pub fn centred_top(height: f32) -> f32 {
    (HEIGHT - height) / 2.0
}

fn rgb(hex: u32) -> D2D1_COLOR_F {
    let channel = |shift: u32| f32::from(u8::try_from((hex >> shift) & 0xFF).unwrap_or(0)) / 255.0;
    D2D1_COLOR_F {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: 1.0,
    }
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

/// Where and how one line of text is drawn.
struct Slot {
    left: f32,
    width: f32,
    style: TextStyle,
    align: DWRITE_TEXT_ALIGNMENT,
}

/// The device brushes and the shared text system; owned by the render thread.
pub struct Painter {
    /// Device pixels per DIP, for snapping.
    scale: f32,
    text: TextSystem,
    /// Log notes from building the text system, drained once by [`Painter::take_notes`].
    notes: Vec<String>,
    floor: ID2D1SolidColorBrush,
    border: ID2D1SolidColorBrush,
    ink: ID2D1SolidColorBrush,
    muted: ID2D1SolidColorBrush,
    faint: ID2D1SolidColorBrush,
    track: ID2D1SolidColorBrush,
    lime: ID2D1SolidColorBrush,
}

impl Painter {
    /// Creates the brushes on `gpu`'s context and the text system, for a surface of `scale`
    /// device pixels per DIP.
    ///
    /// # Errors
    /// Returns the first failing Direct2D or DirectWrite call.
    pub fn new(gpu: &Gpu, scale: f32) -> Result<Self> {
        let ctx = gpu.context();
        let brush = |hex: u32| {
            let color = rgb(hex);
            // SAFETY: the color is a live local for the call.
            unsafe { ctx.CreateSolidColorBrush(&raw const color, None) }
        };
        // SAFETY: a plain state setter on the live context.
        unsafe { ctx.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE) };
        let mut text = TextSystem::new()?;
        let mut notes: Vec<String> = text
            .take_notes()
            .into_iter()
            .map(|note| format!("paint: {note}"))
            .collect();
        if !text.uses_bundled_fonts() && notes.is_empty() {
            notes.push(String::from("paint: bundled fonts not in use"));
        }
        Ok(Self {
            scale,
            text,
            notes,
            floor: brush(FLOOR)?,
            border: brush(BORDER)?,
            ink: brush(INK)?,
            muted: brush(MUTED)?,
            faint: brush(FAINT)?,
            track: brush(TRACK)?,
            lime: brush(LIME)?,
        })
    }

    /// The mono family the text uses.
    #[must_use]
    pub fn family(&self) -> &str {
        self.text.family_name(Family::Mono)
    }

    /// The `paint:` log notes recorded while building; empty after the first call.
    #[must_use]
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    fn set_opacity(&self, opacity: f32) {
        for brush in [
            &self.floor,
            &self.border,
            &self.ink,
            &self.muted,
            &self.faint,
            &self.track,
            &self.lime,
        ] {
            // SAFETY: a plain setter on a live brush.
            unsafe { brush.SetOpacity(opacity) };
        }
    }

    /// Draws `frame` between `BeginDraw` and `EndDraw` (the caller brackets it).
    ///
    /// # Errors
    /// Returns the failing DirectWrite call of the text.
    pub fn draw(&mut self, gpu: &Gpu, frame: &OsdFrame) -> Result<()> {
        let ctx = gpu.context();
        let clear = D2D1_COLOR_F::default();
        // SAFETY: the context targets the swap chain back buffer; `clear` is a live local.
        unsafe { ctx.Clear(Some(&raw const clear)) };
        if frame.opacity <= 0.0 {
            return Ok(());
        }
        self.set_opacity(frame.opacity);
        self.draw_seal(gpu);
        self.draw_ruler(gpu, frame);
        let label = Slot {
            left: PAD,
            width: LABEL_RIGHT - PAD,
            style: TextStyle::Label,
            align: DWRITE_TEXT_ALIGNMENT_LEADING,
        };
        self.draw_text(gpu, &frame.label, &label, &self.muted.clone())?;
        let value = Slot {
            left: VALUE_LEFT,
            width: WIDTH - PAD - VALUE_LEFT,
            style: TextStyle::Value,
            align: DWRITE_TEXT_ALIGNMENT_TRAILING,
        };
        let brush = if frame.muted { &self.faint } else { &self.ink }.clone();
        self.draw_text(gpu, &value_text(frame), &value, &brush)
    }

    /// The floor and its 1 DIP border, whole device pixels wide. The shape starts `RADIUS`
    /// above the surface, so its top corners and top border fall outside it: square top,
    /// round bottom.
    fn draw_seal(&self, gpu: &Gpu) {
        let line = line_px(self.scale) / self.scale;
        let half = line / 2.0;
        let shape = D2D1_ROUNDED_RECT {
            rect: rect(half, -RADIUS, WIDTH - half, HEIGHT - half),
            radiusX: RADIUS,
            radiusY: RADIUS,
        };
        // SAFETY: the shape is a live local; the brushes belong to this context.
        unsafe {
            gpu.context()
                .FillRoundedRectangle(&raw const shape, &self.floor);
            gpu.context()
                .DrawRoundedRectangle(&raw const shape, &self.border, line, None);
        }
    }

    /// The track ([`line_px`] thick), a tick every 10 % below it (all the same width) and the
    /// fill (twice the track, cut when muted), every edge on a device pixel so nothing blurs
    /// at fractional scales.
    fn draw_ruler(&self, gpu: &Gpu, frame: &OsdFrame) {
        let s = self.scale;
        let ctx = gpu.context();
        let (left, right) = (snap(RULER.0, s), snap(RULER.1, s));
        let ((track_top, track_bottom), (fill_top, fill_bottom)) = ruler_rows(s);
        let track = rect(left, track_top, right, track_bottom);
        let (tick_top, tick_bottom) = snap_span(track_bottom, TICK, s);
        // SAFETY: every rect is a live local; the brushes belong to this context.
        unsafe {
            ctx.FillRectangle(&raw const track, &self.track);
            for i in 0..=10 {
                let (x0, x1) = tick_span(i, s);
                let tick = rect(x0, tick_top, x1, tick_bottom);
                ctx.FillRectangle(&raw const tick, &self.track);
            }
            if !frame.muted && frame.level > 0.0 {
                let fill = rect(
                    left,
                    fill_top,
                    snap(fill_right(frame.level), s),
                    fill_bottom,
                );
                let brush = match frame.meter {
                    Meter::Volume => &self.lime,
                    Meter::Brightness => &self.ink,
                };
                ctx.FillRectangle(&raw const fill, brush);
            }
        }
    }

    /// One line of `text` in `slot`, ended with an ellipsis when too long and vertically
    /// centred in the OSD.
    fn draw_text(
        &mut self,
        gpu: &Gpu,
        text: &str,
        slot: &Slot,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let layout = self.text.layout(text, slot.style, slot.width, Trim::End)?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        // SAFETY: the out pointer refers to a live local; the layout and brush are live and
        // the draw call is inside the caller's BeginDraw/EndDraw.
        unsafe {
            layout.GetMetrics(&raw mut metrics)?;
            layout.SetTextAlignment(slot.align)?;
            gpu.context().DrawTextLayout(
                Vector2 {
                    X: slot.left,
                    Y: centred_top(metrics.height),
                },
                &layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
