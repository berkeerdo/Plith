//! The renderer: D3D11, Direct2D, DirectWrite and DirectComposition on a dedicated render thread with a vblank frame clock.
//! This slice holds the device, the composition target, the painter, the frame clocks and the render thread.

pub mod clock;
pub mod device;
pub mod gpu_paint;
pub mod offscreen;
pub mod paint;
pub mod target;
pub mod thread;

pub use clock::{ClockError, FrameSource};
pub use device::Gpu;
pub use gpu_paint::GpuPaint;
pub use offscreen::render_offscreen;
pub use paint::{PaintFrame, Painter};
pub use target::CompositionTarget;
pub use thread::{
    FrameTimes, Paint, RenderConfig, RenderError, RenderHandle, RenderSinks, StopHandle, spawn,
};
