//! The ambient tick: when the open island repaints without motion (meters, seek fill), how
//! often, the values sampled for it, and its `ambient` log line.

use crate::draw::{Node, PageContent};
use crate::scene::Scene;
use std::fmt;
use std::time::SystemTime;

/// A gap between ambient frames above this many ambient intervals is a hitch.
const HITCH_INTERVALS: f64 = 1.5;
/// Draw times are binned in hundredths of a millisecond, the precision the line prints.
const BINS_PER_MS: f64 = 100.0;
/// Draw times at or above this many milliseconds share the last bin.
const BINNED_MS: usize = 50;

/// The ambient rate is the display rate divided by this: about 60 frames per second, at least
/// one tick per frame (4 at 240 Hz, 2 at 120 and 144 Hz, 1 at 60 Hz).
#[must_use]
pub fn ambient_divisor(hz: f64) -> u32 {
    let ratio = (hz / 60.0).round();
    if ratio.is_nan() || ratio < 1.0 {
        return 1;
    }
    // Allowed: the value is a whole number clamped to 1..=1000.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let divisor = ratio.min(1000.0) as u32;
    divisor
}

/// True when the settled island needs ambient frames: it is expanded, nothing suppresses it and
/// the visible page's display list has a live node.
#[must_use]
pub fn wants_ambient(scene: &Scene, content: &PageContent) -> bool {
    if !scene.expanded || scene.suppression.any() {
        return false;
    }
    content
        .pages
        .get(scene.page)
        .is_some_and(|list| list.nodes.iter().any(|node| matches!(node, Node::Live(_))))
}

/// The recent level history of one meter, oldest first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TraceSamples(pub Vec<f32>);

/// Values sampled once per ambient frame for the live nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveValues {
    /// Left and right levels of each input strip.
    pub strips: Vec<[f32; 2]>,
    /// Left and right levels of each output bus.
    pub buses: Vec<[f32; 2]>,
    /// Level histories for trace nodes.
    pub traces: Vec<TraceSamples>,
    /// Wall time of the sample, for extrapolating the seek position.
    pub now: SystemTime,
}

impl Default for LiveValues {
    fn default() -> Self {
        Self {
            strips: Vec::new(),
            buses: Vec::new(),
            traces: Vec::new(),
            now: SystemTime::UNIX_EPOCH,
        }
    }
}

/// Final numbers of one ambient run, ready to log.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientReport {
    /// The page shown.
    pub page: usize,
    /// Seconds from the first ambient frame to one interval after the last.
    pub secs: f64,
    /// Frames painted.
    pub frames: usize,
    /// Gaps over 1.5 ambient intervals.
    pub hitches: usize,
    /// Largest gap between consecutive frames, in milliseconds.
    pub worst_gap_ms: f64,
    /// 99th percentile draw time in milliseconds, to a hundredth.
    pub draw_p99_ms: f64,
}

impl fmt::Display for AmbientReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ambient: page {} {:.1} s, {} frames, {} hitches, worst {:.2} ms, draw p99 {:.2} ms",
            self.page, self.secs, self.frames, self.hitches, self.worst_gap_ms, self.draw_p99_ms
        )
    }
}

/// Collects the frames of one ambient run in fixed memory, however long it lasts.
#[derive(Debug, Clone)]
pub struct AmbientStats {
    page: usize,
    interval: f64,
    first: f64,
    last: f64,
    frames: usize,
    hitches: usize,
    worst_gap: f64,
    draw_bins: Vec<u32>,
}

impl AmbientStats {
    /// Starts collecting for `page` on a display of `hz`.
    #[must_use]
    pub fn new(page: usize, hz: f64) -> Self {
        Self {
            page,
            interval: f64::from(ambient_divisor(hz)) / hz,
            first: 0.0,
            last: 0.0,
            frames: 0,
            hitches: 0,
            worst_gap: 0.0,
            draw_bins: vec![0; draw_bin_count()],
        }
    }

    /// Records one painted frame at animation time `anim_t` that took `draw_ms` to draw.
    pub fn record_frame(&mut self, anim_t: f64, draw_ms: f64) {
        if self.frames == 0 {
            self.first = anim_t;
        } else {
            let gap = anim_t - self.last;
            self.worst_gap = self.worst_gap.max(gap);
            if gap > HITCH_INTERVALS * self.interval {
                self.hitches += 1;
            }
        }
        self.last = anim_t;
        self.frames += 1;
        let bin = draw_bin(draw_ms).min(self.draw_bins.len() - 1);
        self.draw_bins[bin] = self.draw_bins[bin].saturating_add(1);
    }

    /// Frames recorded so far.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Summarises the frames recorded so far.
    #[must_use]
    pub fn finish(&self) -> AmbientReport {
        let secs = if self.frames == 0 {
            0.0
        } else {
            self.last - self.first + self.interval
        };
        AmbientReport {
            page: self.page,
            secs,
            frames: self.frames,
            hitches: self.hitches,
            worst_gap_ms: self.worst_gap * 1000.0,
            draw_p99_ms: self.draw_p99(),
        }
    }

    /// Nearest-rank 99th percentile over the bins; 0 when no frame was recorded.
    fn draw_p99(&self) -> f64 {
        if self.frames == 0 {
            return 0.0;
        }
        let rank = self.frames - self.frames / 100;
        let mut seen = 0_usize;
        for (bin, count) in self.draw_bins.iter().enumerate() {
            seen += *count as usize;
            if seen >= rank {
                return bin_ms(bin);
            }
        }
        bin_ms(self.draw_bins.len() - 1)
    }
}

fn draw_bin_count() -> usize {
    BINNED_MS * 100 + 1
}

/// The bin of a draw time: hundredths of a millisecond, rounded; negative times go to bin 0.
// Allowed: the value is rounded, clamped to 0 and capped below `usize::MAX` by the caller's min.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn draw_bin(draw_ms: f64) -> usize {
    let hundredths = (draw_ms.max(0.0) * BINS_PER_MS).round();
    hundredths.min(1e9) as usize
}

/// The draw time a bin stands for.
// Allowed: bin indices are at most a few thousand, exact in f64.
#[allow(clippy::cast_precision_loss)]
fn bin_ms(bin: usize) -> f64 {
    bin as f64 / BINS_PER_MS
}

#[cfg(test)]
mod tests;
