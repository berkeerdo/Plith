//! Draws one OSD frame: the seal (square top, round bottom), the label, the ruler and the value,
//! in three fixed slots that never reflow. Every length is in DIP; the target's DPI scales it.

mod text;

use crate::model::OsdFrame;
use plith_ipc::Meter;
use plith_render::Gpu;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_MEASURING_MODE_NATURAL, IDWriteTextFormat};
use windows::core::Result;

pub use text::Fonts;

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
const RULER: (f32, f32) = (92.0, 236.0);
/// Right slot (value) left edge.
const VALUE_LEFT: f32 = 244.0;
/// Tick length below the track.
const TICK: f32 = 3.0;
/// The word shown in the value slot while muted.
pub const MUTED_WORD: &str = "SESSİZ";

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
        return String::from(MUTED_WORD);
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// The device brushes and text formats; owned by the render thread.
pub struct Painter {
    fonts: Fonts,
    floor: ID2D1SolidColorBrush,
    border: ID2D1SolidColorBrush,
    ink: ID2D1SolidColorBrush,
    muted: ID2D1SolidColorBrush,
    faint: ID2D1SolidColorBrush,
    track: ID2D1SolidColorBrush,
    lime: ID2D1SolidColorBrush,
}

impl Painter {
    /// Creates the brushes on `gpu`'s context and the text formats.
    ///
    /// # Errors
    /// Returns the first failing Direct2D or DirectWrite call.
    pub fn new(gpu: &Gpu) -> Result<Self> {
        let ctx = gpu.context();
        let brush = |hex: u32| {
            let color = rgb(hex);
            // SAFETY: the color is a live local for the call.
            unsafe { ctx.CreateSolidColorBrush(&raw const color, None) }
        };
        // SAFETY: a plain state setter on the live context.
        unsafe { ctx.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE) };
        Ok(Self {
            fonts: Fonts::new()?,
            floor: brush(FLOOR)?,
            border: brush(BORDER)?,
            ink: brush(INK)?,
            muted: brush(MUTED)?,
            faint: brush(FAINT)?,
            track: brush(TRACK)?,
            lime: brush(LIME)?,
        })
    }

    /// The mono family the text formats use.
    #[must_use]
    pub fn family(&self) -> &'static str {
        self.fonts.family
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
    pub fn draw(&self, gpu: &Gpu, frame: &OsdFrame) {
        let ctx = gpu.context();
        let clear = D2D1_COLOR_F::default();
        // SAFETY: the context targets the swap chain back buffer; `clear` is a live local.
        unsafe { ctx.Clear(Some(&raw const clear)) };
        if frame.opacity <= 0.0 {
            return;
        }
        self.set_opacity(frame.opacity);
        self.draw_seal(gpu);
        self.draw_ruler(gpu, frame);
        let label = frame.label.to_uppercase();
        Self::text(
            gpu,
            &label,
            &self.fonts.label,
            (PAD, LABEL_RIGHT),
            &self.muted,
        );
        let value = value_text(frame);
        let (format, brush) = if frame.muted {
            (&self.fonts.mute, &self.faint)
        } else {
            (&self.fonts.value, &self.ink)
        };
        Self::text(gpu, &value, format, (VALUE_LEFT, WIDTH - PAD), brush);
    }

    /// The floor and its 1 DIP border. The shape starts `RADIUS` above the surface, so its top
    /// corners and top border fall outside it: square top, round bottom.
    fn draw_seal(&self, gpu: &Gpu) {
        let shape = D2D1_ROUNDED_RECT {
            rect: rect(0.5, -RADIUS, WIDTH - 0.5, HEIGHT - 0.5),
            radiusX: RADIUS,
            radiusY: RADIUS,
        };
        // SAFETY: the shape is a live local; the brushes belong to this context.
        unsafe {
            gpu.context()
                .FillRoundedRectangle(&raw const shape, &self.floor);
            gpu.context()
                .DrawRoundedRectangle(&raw const shape, &self.border, 1.0, None);
        }
    }

    /// The 1 DIP track, a tick every 10 % below it and the 2 DIP fill (cut when muted).
    fn draw_ruler(&self, gpu: &Gpu, frame: &OsdFrame) {
        let mid = HEIGHT / 2.0;
        let ctx = gpu.context();
        let track = rect(RULER.0, mid - 0.5, RULER.1, mid + 0.5);
        // SAFETY: every rect is a live local; the brushes belong to this context.
        unsafe {
            ctx.FillRectangle(&raw const track, &self.track);
            for i in 0..=10 {
                let x = tick_x(i);
                let tick = rect(x - 0.5, mid + 0.5, x + 0.5, mid + 0.5 + TICK);
                ctx.FillRectangle(&raw const tick, &self.track);
            }
            if !frame.muted && frame.level > 0.0 {
                let fill = rect(RULER.0, mid - 1.0, fill_right(frame.level), mid + 1.0);
                let brush = match frame.meter {
                    Meter::Volume => &self.lime,
                    Meter::Brightness => &self.ink,
                };
                ctx.FillRectangle(&raw const fill, brush);
            }
        }
    }

    fn text(
        gpu: &Gpu,
        text: &str,
        format: &IDWriteTextFormat,
        (left, right): (f32, f32),
        brush: &ID2D1SolidColorBrush,
    ) {
        let layout = rect(left, 0.0, right, HEIGHT);
        // SAFETY: the text, format, rect and brush are live for the call.
        unsafe {
            gpu.context().DrawText(
                &wide(text),
                format,
                &raw const layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

#[cfg(test)]
mod tests;
