//! The morph progress as a line through two (height, progress) points, so a retarget can start
//! the new line at the progress the island has now: a reopen during a collapse or a page switch
//! moves on from where it is, with no jump, and still reaches 1 at its target.

/// Progress as a function of the island's height: linear between two (height, progress)
/// points, clamped to the segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphLine {
    from: (f64, f64),
    to: (f64, f64),
}

impl MorphLine {
    /// An open from rest: 0 at `rest`, 1 at `open` (the same as `morph_progress`).
    #[must_use]
    pub fn open(rest: f64, open: f64) -> Self {
        Self {
            from: (rest, 0.0),
            to: (open, 1.0),
        }
    }

    /// The progress at `height`.
    #[must_use]
    pub fn progress(&self, height: f64) -> f64 {
        let ((h0, p0), (h1, p1)) = (self.from, self.to);
        if (h1 - h0).abs() < 1e-9 {
            return p1;
        }
        let t = ((height - h0) / (h1 - h0)).clamp(0.0, 1.0);
        (p0 + (p1 - p0) * t).clamp(0.0, 1.0)
    }

    /// An open (or a page switch) toward `target` from the island's `height`: the progress at
    /// `height` now, 1 at `target`.
    #[must_use]
    pub fn toward(&self, height: f64, target: f64) -> Self {
        Self {
            from: (height, self.progress(height)),
            to: (target, 1.0),
        }
    }

    /// A collapse from the island's `height`: the progress at `height` now, 0 at `rest`.
    #[must_use]
    pub fn collapse(&self, height: f64, rest: f64) -> Self {
        Self {
            from: (rest, 0.0),
            to: (height, self.progress(height)),
        }
    }
}
