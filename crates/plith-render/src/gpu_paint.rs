//! The real [`Paint`]: device, composition target and painter, timed with `Instant`.

use crate::device::Gpu;
use crate::levers::apply_gpu_priority;
use crate::paint::{PaintFrame, Painter};
use crate::target::CompositionTarget;
use crate::text::TextSystem;
use crate::thread::{FrameTimes, Paint};
use plith_core::draw::PageContent;
use plith_core::layout::SurfaceLayout;
use plith_core::levers::{Levers, outcome_line};
use plith_core::theme::Palette;
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
        palette: Palette,
    ) -> Result<(Self, Vec<String>)> {
        let gpu = Gpu::new(warp)?;
        let mut target = CompositionTarget::new(&gpu, hwnd, layout, levers)?;
        let mut notes: Vec<String> = apply_gpu_priority(gpu.dxgi_device(), levers.gpu_priority)
            .into_iter()
            .chain(target.take_outcome())
            .map(|outcome| outcome_line(&outcome))
            .collect();
        let text = TextSystem::new()?;
        let mut painter = Painter::new(gpu.context(), gpu.d2d_factory(), palette, text)?;
        notes.extend(painter.take_notes());
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
    /// The palette the painter uses; kept here so a recovered painter gets it back.
    palette: Palette,
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
    pub fn new(
        hwnd: HWND,
        layout: SurfaceLayout,
        warp: bool,
        levers: Levers,
        palette: Palette,
    ) -> Result<Self> {
        let (stack, notes) = Stack::new(hwnd, &layout, warp, levers, palette)?;
        Ok(Self {
            hwnd,
            layout,
            levers,
            palette,
            stack: Some(stack),
            notes,
        })
    }
}

impl Paint for GpuPaint {
    /// Switches the palette now and for every painter rebuilt by a recovery.
    fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
        if let Some(stack) = self.stack.as_mut() {
            stack.painter.set_palette(palette);
        }
    }

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

    fn prewarm(&mut self, content: &PageContent, pages: &[usize]) -> f64 {
        let Some(stack) = self.stack.as_mut() else {
            return 0.0;
        };
        let before = stack.painter.page_rebuilds();
        let start = Instant::now();
        // A failure (device lost) shows again at the next paint, which reports it to the GPU
        // health; the page is then rastered lazily.
        let _ = stack.painter.prewarm(stack.gpu.context(), content, pages);
        if stack.painter.page_rebuilds() == before {
            0.0
        } else {
            ms_since(start)
        }
    }

    fn relayout(&mut self, layout: SurfaceLayout) -> Result<()> {
        // The swap chain, the page bitmaps and the text formats all depend on the size and the
        // scale: the device-loss rebuild makes them again (a display change is rare).
        self.layout = layout;
        self.recover(false)
    }

    fn recover(&mut self, warp: bool) -> Result<()> {
        if let Some(stack) = self.stack.take() {
            stack.target.release_target(&stack.gpu);
            // Dropped before the rebuild: a window takes only one composition target.
            drop(stack);
        }
        let (stack, notes) = Stack::new(self.hwnd, &self.layout, warp, self.levers, self.palette)?;
        self.stack = Some(stack);
        self.notes.extend(notes);
        Ok(())
    }

    fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }
}
