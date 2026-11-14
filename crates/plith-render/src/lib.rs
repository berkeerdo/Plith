//! The renderer: D3D11, Direct2D, DirectWrite and DirectComposition on a dedicated render thread with a vblank frame clock.
//! This slice holds the device, the composition target and the painter; the render thread comes later.

pub mod device;
pub mod offscreen;
pub mod paint;
pub mod target;

pub use device::Gpu;
pub use offscreen::render_offscreen;
pub use paint::{PaintFrame, Painter};
pub use target::CompositionTarget;
