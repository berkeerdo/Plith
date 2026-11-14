//! The real [`Paint`]: device, composition target and painter, timed with `Instant`.

use crate::device::Gpu;
use crate::paint::{PaintFrame, Painter};
use crate::target::CompositionTarget;
use crate::thread::{FrameTimes, Paint};
use plith_core::layout::SurfaceLayout;
use std::time::Instant;
use windows::Win32::Foundation::{E_UNEXPECTED, HWND};
use windows::core::{Error, Result};

/// The objects rebuilt together on recovery.
struct Stack {
    gpu: Gpu,
    target: CompositionTarget,
    painter: Painter,
}

impl Stack {
    fn new(hwnd: HWND, layout: &SurfaceLayout, warp: bool) -> Result<Self> {
        let gpu = Gpu::new(warp)?;
        let target = CompositionTarget::new(&gpu, hwnd, layout)?;
        let painter = Painter::new(gpu.context())?;
        Ok(Self {
            gpu,
            target,
            painter,
        })
    }
}

/// Paints onto the notch window through DirectComposition. Lives on the render thread only.
pub struct GpuPaint {
    hwnd: HWND,
    layout: SurfaceLayout,
    stack: Option<Stack>,
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

impl GpuPaint {
    /// Creates the device stack for `hwnd` (on WARP when `warp` is set).
    ///
    /// # Errors
    /// Returns the first failing creation call.
    pub fn new(hwnd: HWND, layout: SurfaceLayout, warp: bool) -> Result<Self> {
        Ok(Self {
            hwnd,
            layout,
            stack: Some(Stack::new(hwnd, &layout, warp)?),
        })
    }
}

impl Paint for GpuPaint {
    fn paint(&mut self, frame: &PaintFrame) -> Result<FrameTimes> {
        let stack = self
            .stack
            .as_mut()
            .ok_or_else(|| Error::from(E_UNEXPECTED))?;
        let start = Instant::now();
        stack.painter.paint(stack.gpu.context(), frame)?;
        let draw_ms = ms_since(start);
        let start = Instant::now();
        stack.target.present()?;
        Ok(FrameTimes {
            draw_ms,
            present_ms: ms_since(start),
        })
    }

    fn recover(&mut self, warp: bool) -> Result<()> {
        if let Some(stack) = self.stack.take() {
            stack.target.release_target(&stack.gpu);
            // Dropped before the rebuild: a window takes only one composition target.
            drop(stack);
        }
        self.stack = Some(Stack::new(self.hwnd, &self.layout, warp)?);
        Ok(())
    }
}
