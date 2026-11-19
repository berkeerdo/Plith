//! The ambient tick: when the open island repaints without motion (meters, seek fill), how
//! often, the values sampled for it, and its `ambient` log line.

use crate::draw::{DisplayList, LiveNode, LiveRail, Node, PageContent};
use crate::page::PageLayers;
use crate::scene::Scene;
use std::fmt;
use std::time::{Duration, SystemTime};

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

/// True when the settled island needs ambient frames and nothing suppresses it: it is expanded
/// and the visible page's display list has a live node, or it is collapsed and the pill has a
/// title scroll running at `now` (`start <= now < start + run_s`).
#[must_use]
pub fn wants_ambient(scene: &Scene, content: &PageContent, now: SystemTime) -> bool {
    if scene.suppression.any() {
        return false;
    }
    if scene.expanded {
        return content
            .pages
            .get(scene.page)
            .is_some_and(|list| has_live(list));
    }
    content.pill.nodes.iter().any(|node| match node {
        Node::Live(LiveNode::Scroll { start, run_s, .. }) => scroll_running(*start, *run_s, now),
        _ => false,
    })
}

/// True while `start <= now < start + run_s`. A `now` before `start` (the wall clock stepped
/// back) is not running: the painter draws the rest offset there, so frames would show nothing
/// new for as long as the step.
fn scroll_running(start: SystemTime, run_s: f64, now: SystemTime) -> bool {
    let Ok(run) = Duration::try_from_secs_f64(run_s) else {
        return false;
    };
    now.duration_since(start).is_ok_and(|elapsed| elapsed < run)
}

fn has_live(list: &DisplayList) -> bool {
    list.nodes.iter().any(|node| matches!(node, Node::Live(_)))
}

/// True when a frame drawn with `layers` at `content_opacity` shows a live node: the content
/// is visible and some visible layer's page (incoming or outgoing) has one. Such a frame needs
/// sampled live values, whether it is a motion, a static repaint or an ambient frame.
#[must_use]
pub fn shows_live(content: &PageContent, layers: &PageLayers, content_opacity: f64) -> bool {
    content_opacity > 0.0
        && layers.iter().any(|layer| {
            layer.opacity > 0.0 && content.pages.get(layer.page).is_some_and(|l| has_live(l))
        })
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

impl LiveValues {
    /// The louder side of meter `index` of `rail`; 0 when there is no such meter.
    #[must_use]
    pub fn peak(&self, rail: LiveRail, index: usize) -> f32 {
        let family = match rail {
            LiveRail::Strip => &self.strips,
            LiveRail::Bus => &self.buses,
        };
        family.get(index).map_or(0.0, |[l, r]| l.max(*r))
    }

    /// The samples of trace `index`, oldest first; empty when there is no such trace.
    #[must_use]
    pub fn trace(&self, index: usize) -> &[f32] {
        self.traces.get(index).map_or(&[], |samples| &samples.0)
    }
}

/// What an ambient run painted: a page of the open island, or the collapsed pill's title
/// scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientTarget {
    /// The open island showing this page; painted every `ambient_divisor(hz)`-th tick.
    Page(usize),
    /// The collapsed pill's title scroll; painted every tick, at the display rate.
    Pill,
}

impl fmt::Display for AmbientTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page(page) => write!(f, "page {page}"),
            Self::Pill => f.write_str("pill"),
        }
    }
}

/// Final numbers of one ambient run, ready to log.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientReport {
    /// What the run painted.
    pub target: AmbientTarget,
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
            "ambient: {} {:.1} s, {} frames, {} hitches, worst {:.2} ms, draw p99 {:.2} ms",
            self.target, self.secs, self.frames, self.hitches, self.worst_gap_ms, self.draw_p99_ms
        )
    }
}

/// Collects the frames of one ambient run in fixed memory, however long it lasts.
#[derive(Debug, Clone)]
pub struct AmbientStats {
    target: AmbientTarget,
    divisor: u32,
    interval: f64,
    first: f64,
    last: f64,
    frames: usize,
    hitches: usize,
    worst_gap: f64,
    draw_bins: Vec<u32>,
}

impl AmbientStats {
    /// Starts collecting for `page` on a display of `hz`, painted at the ambient rate.
    #[must_use]
    pub fn new(page: usize, hz: f64) -> Self {
        Self::of(AmbientTarget::Page(page), ambient_divisor(hz), hz)
    }

    /// Starts collecting for the collapsed pill's title scroll on a display of `hz`, painted on
    /// every tick so the scroll moves at the display rate.
    #[must_use]
    pub fn pill(hz: f64) -> Self {
        Self::of(AmbientTarget::Pill, 1, hz)
    }

    fn of(target: AmbientTarget, divisor: u32, hz: f64) -> Self {
        Self {
            target,
            divisor,
            interval: f64::from(divisor) / hz,
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

    /// Clock ticks per painted frame in this run: 1 for the pill, `ambient_divisor(hz)` for a
    /// page.
    #[must_use]
    pub fn divisor(&self) -> u32 {
        self.divisor
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
            target: self.target,
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
