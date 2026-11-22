//! What the notch is laid out for: the surface, the monitor, its refresh rate and whether the
//! session is remote. A display change compares two of these.

use crate::layout::SurfaceLayout;

/// The display state the window, the hit region and the render thread follow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// The surface rectangle and DPI scale.
    pub layout: SurfaceLayout,
    /// The primary monitor's `HMONITOR` as an integer.
    pub monitor: isize,
    /// Nominal refresh rate in hertz.
    pub hz: f64,
    /// True in a remote session (timer clock only).
    pub remote: bool,
}

impl Display {
    /// True when the frame clock must be reopened for `next`: another monitor, another
    /// refresh rate, or the session turned remote or local.
    #[must_use]
    pub fn clock_changed(&self, next: &Self) -> bool {
        #[allow(clippy::float_cmp)]
        let same_hz = self.hz == next.hz;
        self.monitor != next.monitor || !same_hz || self.remote != next.remote
    }

    /// The log line for a relayout to this display.
    #[must_use]
    pub fn log_line(&self) -> String {
        let r = self.layout.surface;
        format!(
            "relayout: rect={},{} {}x{} scale={:.2} hz={:.2} remote={}",
            r.x, r.y, r.width, r.height, self.layout.scale, self.hz, self.remote
        )
    }
}

#[cfg(test)]
mod tests;
