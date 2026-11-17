//! The graphics objects of the render thread, built together and rebuilt together after a
//! device loss (a game's driver reset must not end the OSD).

use crate::model::OsdFrame;
use crate::paint::Painter;
use plith_core::layout::SurfaceLayout;
use plith_core::levers::Levers;
use plith_render::{CompositionTarget, Gpu};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND};
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
};
use windows::core::{Error, HRESULT};

/// Whether `code` means the device or the target is gone and everything must be rebuilt.
#[must_use]
pub fn is_device_lost(code: HRESULT) -> bool {
    [
        DXGI_ERROR_DEVICE_REMOVED,
        DXGI_ERROR_DEVICE_RESET,
        DXGI_ERROR_DEVICE_HUNG,
        D2DERR_RECREATE_TARGET,
    ]
    .contains(&code)
}

/// Device, swap chain target and painter. Fields drop in order: the painter's brushes, then
/// the target, then the device.
pub struct Stack {
    painter: Painter,
    target: CompositionTarget,
    gpu: Gpu,
}

impl Stack {
    /// Builds everything for `hwnd`, on WARP when `warp` is true.
    ///
    /// # Errors
    /// Returns the failing step and its error.
    pub fn build(hwnd: HWND, layout: &SurfaceLayout, warp: bool) -> Result<Self, String> {
        let gpu = Gpu::new(warp).map_err(|e| format!("Gpu::new: {e}"))?;
        let target = CompositionTarget::new(&gpu, hwnd, layout, Levers::default())
            .map_err(|e| format!("CompositionTarget::new: {e}"))?;
        // Allowed: a DPI scale is a small positive number, far inside f32 range.
        #[allow(clippy::cast_possible_truncation)]
        let scale = layout.scale as f32;
        let painter = Painter::new(&gpu, scale).map_err(|e| format!("painter: {e}"))?;
        Ok(Self {
            painter,
            target,
            gpu,
        })
    }

    /// Whether the device is the WARP software rasterizer.
    #[must_use]
    pub fn is_warp(&self) -> bool {
        self.gpu.is_warp()
    }

    /// The mono family the painter uses.
    #[must_use]
    pub fn family(&self) -> &str {
        self.painter.family()
    }

    /// The `paint:` notes recorded while building the painter, for the log.
    pub fn take_notes(&mut self) -> Vec<String> {
        self.painter.take_notes()
    }

    /// Draws `frame` and presents it. Returns draw and present times in ms.
    ///
    /// # Errors
    /// Returns the `EndDraw` or `Present` error; [`is_device_lost`] tells a loss.
    pub fn paint(&mut self, frame: &OsdFrame) -> Result<(f64, f64), Error> {
        let start = Instant::now();
        // SAFETY: BeginDraw on the live context; EndDraw follows on every path.
        unsafe { self.gpu.context().BeginDraw() };
        let drawn = self.painter.draw(&self.gpu, frame);
        // SAFETY: closes the BeginDraw above, on every path.
        unsafe { self.gpu.context().EndDraw(None, None) }?;
        drawn?;
        let drawn = Instant::now();
        self.target.present()?;
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        Ok((ms(drawn - start), ms(drawn.elapsed())))
    }
}

#[cfg(test)]
mod tests;
