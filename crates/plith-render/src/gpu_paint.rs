//! The real [`Paint`]: device, composition target and painter, timed with `Instant`.

use crate::device::Gpu;
use crate::levers::apply_gpu_priority;
use crate::paint::{PaintFrame, Painter};
use crate::target::CompositionTarget;
use crate::thread::{FrameTimes, Paint};
use plith_core::layout::SurfaceLayout;
use plith_core::levers::{Levers, outcome_line};
use std::time::Instant;
use windows::Win32::Foundation::{E_UNEXPECTED, HWND};
use windows::core::{Error, Result};

/// The objects rebuilt together on recovery. Fields drop in declaration order, so the painter
/// (which holds views of the device) goes first and the device last.
struct Stack {
    painter: Painter,
    target: CompositionTarget,
    gpu: Gpu,
}

impl Stack {
    /// Builds the stack; returns it with the log lines of the levers it applied.
    fn new(
        hwnd: HWND,
        layout: &SurfaceLayout,
        warp: bool,
        levers: Levers,
    ) -> Result<(Self, Vec<String>)> {
        let gpu = Gpu::new(warp)?;
        let mut target = CompositionTarget::new(&gpu, hwnd, layout, levers)?;
        let notes = apply_gpu_priority(gpu.dxgi_device(), levers.gpu_priority)
            .into_iter()
            .chain(target.take_outcome())
            .map(|outcome| outcome_line(&outcome))
            .collect();
        let painter = Painter::new(gpu.context(), gpu.d2d_factory())?;
        Ok((
            Self {
                painter,
                target,
                gpu,
            },
            notes,
        ))
    }
}

/// Paints onto the notch window through DirectComposition. Lives on the render thread only.
pub struct GpuPaint {
    hwnd: HWND,
    layout: SurfaceLayout,
    levers: Levers,
    stack: Option<Stack>,
    notes: Vec<String>,
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

impl GpuPaint {
    /// Creates the device stack for `hwnd` (on WARP when `warp` is set).
    ///
    /// # Errors
    /// Returns the first failing creation call.
    pub fn new(hwnd: HWND, layout: SurfaceLayout, warp: bool, levers: Levers) -> Result<Self> {
        let (stack, notes) = Stack::new(hwnd, &layout, warp, levers)?;
        Ok(Self {
            hwnd,
            layout,
            levers,
            stack: Some(stack),
            notes,
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
        let (stack, notes) = Stack::new(self.hwnd, &self.layout, warp, self.levers)?;
        self.stack = Some(stack);
        self.notes.extend(notes);
        Ok(())
    }

    fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }
}
