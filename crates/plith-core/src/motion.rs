//! Per-motion frame statistics and the `motion` log line.

use crate::geometry::IslandShape;
use crate::shapes::REST;
use std::fmt;

/// A gap between frames above this many periods is a hitch.
const HITCH_PERIODS: f64 = 1.5;

/// What a motion does to the island.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    /// Rest to an open page.
    Expand,
    /// An open page back to rest.
    Collapse,
    /// Page to page.
    Page,
    /// Any other size change.
    Resize,
}

impl MotionKind {
    /// The word used in the log line.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::Page => "page",
            Self::Resize => "resize",
        }
    }

    /// Classifies a motion from its start and end shapes.
    #[must_use]
    pub fn classify(from: IslandShape, to: IslandShape, page_changed: bool) -> Self {
        if to == REST {
            Self::Collapse
        } else if from == REST {
            Self::Expand
        } else if page_changed {
            Self::Page
        } else {
            Self::Resize
        }
    }
}

/// Final numbers of one motion, ready to log.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionReport {
    /// Kind of motion.
    pub kind: MotionKind,
    /// Frames recorded.
    pub frames: usize,
    /// Duration in milliseconds.
    pub duration_ms: f64,
    /// Frames per second over the duration.
    pub fps: f64,
    /// Display refresh rate.
    pub hz: f64,
    /// Frame gaps over 1.5 periods.
    pub hitches: usize,
    /// Largest gap between consecutive frames, in milliseconds.
    pub worst_gap_ms: f64,
    /// Median draw time in milliseconds.
    pub draw_p50: f64,
    /// 99th percentile draw time in milliseconds.
    pub draw_p99: f64,
    /// Median present time in milliseconds.
    pub present_p50: f64,
    /// 99th percentile present time in milliseconds.
    pub present_p99: f64,
    /// Name of the clock that paced the motion.
    pub clock: &'static str,
    /// Times the motion target changed mid-flight.
    pub retargets: usize,
    /// Duration of the first clock wait of the motion, in milliseconds (0 when none was noted).
    pub first_wait_ms: f64,
}

impl fmt::Display for MotionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "motion {}: {} frames in {:.0} ms ({:.1} fps at {:.0} Hz), {} hitches, \
             worst gap {:.1} ms; draw p50 {:.2} p99 {:.2} ms; \
             present p50 {:.2} p99 {:.2} ms; clock {}; retargets {}; first wait {:.1} ms",
            self.kind.as_str(),
            self.frames,
            self.duration_ms,
            self.fps,
            self.hz,
            self.hitches,
            self.worst_gap_ms,
            self.draw_p50,
            self.draw_p99,
            self.present_p50,
            self.present_p99,
            self.clock,
            self.retargets,
            self.first_wait_ms,
        )
    }
}

/// Collects frame times of one motion.
#[derive(Debug, Clone)]
pub struct MotionStats {
    kind: MotionKind,
    hz: f64,
    clock: &'static str,
    first_anim: f64,
    last_anim: f64,
    frames: usize,
    hitches: usize,
    worst_gap: f64,
    draw: Vec<f64>,
    present: Vec<f64>,
    retargets: usize,
    first_wait_ms: Option<f64>,
}

impl MotionStats {
    /// Starts collecting for a motion on a display of `hz`, paced by `clock`.
    #[must_use]
    pub fn new(kind: MotionKind, hz: f64, clock: &'static str) -> Self {
        Self {
            kind,
            hz,
            clock,
            first_anim: 0.0,
            last_anim: 0.0,
            frames: 0,
            hitches: 0,
            worst_gap: 0.0,
            draw: Vec::new(),
            present: Vec::new(),
            retargets: 0,
            first_wait_ms: None,
        }
    }

    /// Records one presented frame: its animation time and draw and present cost in ms.
    pub fn record_frame(&mut self, anim_t: f64, draw_ms: f64, present_ms: f64) {
        if self.frames == 0 {
            self.first_anim = anim_t;
        } else {
            let gap = anim_t - self.last_anim;
            self.worst_gap = self.worst_gap.max(gap);
            if gap > HITCH_PERIODS / self.hz {
                self.hitches += 1;
            }
        }
        self.last_anim = anim_t;
        self.frames += 1;
        self.draw.push(draw_ms);
        self.present.push(present_ms);
    }

    /// Notes that the motion target changed while the motion was running.
    pub fn note_retarget(&mut self) {
        self.retargets += 1;
    }

    /// Notes the duration of the motion's first clock wait; later calls are ignored.
    pub fn note_first_wait(&mut self, ms: f64) {
        self.first_wait_ms.get_or_insert(ms);
    }

    /// Summarises the frames recorded so far.
    #[must_use]
    pub fn finish(&self) -> MotionReport {
        let duration = if self.frames == 0 {
            0.0
        } else {
            self.last_anim - self.first_anim + 1.0 / self.hz
        };
        let fps = if duration > 0.0 {
            frames_f64(self.frames) / duration
        } else {
            0.0
        };
        let draw = sorted(&self.draw);
        let present = sorted(&self.present);
        MotionReport {
            kind: self.kind,
            frames: self.frames,
            duration_ms: duration * 1000.0,
            fps,
            hz: self.hz,
            hitches: self.hitches,
            worst_gap_ms: self.worst_gap * 1000.0,
            draw_p50: percentile(&draw, 50.0),
            draw_p99: percentile(&draw, 99.0),
            present_p50: percentile(&present, 50.0),
            present_p99: percentile(&present, 99.0),
            clock: self.clock,
            retargets: self.retargets,
            first_wait_ms: self.first_wait_ms.unwrap_or(0.0),
        }
    }
}

fn sorted(values: &[f64]) -> Vec<f64> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v
}

/// Nearest-rank percentile of already sorted values; 0 when empty.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * frames_f64(sorted.len())).ceil();
    sorted[rank_to_index(rank).min(sorted.len() - 1)]
}

/// Frame counts are far below 2^52, so the conversion is exact.
// Allowed: counts stay far below 2^52.
#[allow(clippy::cast_precision_loss)]
fn frames_f64(n: usize) -> f64 {
    n as f64
}

/// Turns a 1-based, non-negative rank into a 0-based index.
// Allowed: rank is a ceiled, non-negative count that fits usize.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rank_to_index(rank: f64) -> usize {
    (rank as usize).saturating_sub(1)
}

#[cfg(test)]
mod tests;
