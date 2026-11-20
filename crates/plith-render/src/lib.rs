//! The renderer: D3D11, Direct2D, DirectWrite and DirectComposition on a dedicated render thread with a vblank frame clock.
//! This slice holds the device, the composition target, the painter, the frame clocks and the render thread.

pub mod clock;
pub mod device;
pub mod gpu_paint;
pub mod levers;
mod list;
mod live;
mod measure;
pub mod offscreen;
mod pages;
pub mod paint;
pub mod sheet;
pub mod target;
pub mod text;
pub mod thread;

pub use clock::{ClockError, FrameSource};
pub use device::Gpu;
pub use gpu_paint::GpuPaint;
pub use measure::TextMeasure;
pub use offscreen::render_offscreen;
pub use paint::{PaintFrame, Painter};
pub use sheet::{SheetConfig, SheetRender, render_sheet_offscreen};
pub use target::CompositionTarget;
pub use text::TextSystem;
pub use thread::{
    Fatal, FrameTimes, Paint, RenderConfig, RenderError, RenderHandle, RenderSinks, StopHandle,
    spawn,
};
