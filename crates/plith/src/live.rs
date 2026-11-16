//! The render thread's `sample` closure: the current levels and one 8 s trace ring per strip.

use crate::services::LevelSource;
use plith_core::ambient::{LiveValues, TraceSamples};
use plith_core::trace::TraceRing;
use std::time::{Instant, SystemTime};

/// Reads levels and keeps the trace history; owned by the render thread.
pub struct Sampler {
    source: LevelSource,
    rings: Vec<TraceRing>,
}

impl Sampler {
    /// A sampler with empty histories.
    #[must_use]
    pub fn new(source: LevelSource) -> Self {
        Self {
            source,
            rings: Vec::new(),
        }
    }

    /// The live values at `now_s` seconds of sampler time and wall time `now`. Without a mixer
    /// only `now` is set.
    pub fn sample_at(&mut self, now_s: f64, now: SystemTime) -> LiveValues {
        let Some(levels) = (self.source)() else {
            return LiveValues {
                now,
                ..LiveValues::default()
            };
        };
        self.rings.resize_with(levels.strips.len(), TraceRing::new);
        for (ring, [left, right]) in self.rings.iter_mut().zip(&levels.strips) {
            ring.push(now_s, left.max(*right));
        }
        LiveValues {
            traces: self
                .rings
                .iter()
                .map(|ring| TraceSamples(ring.samples()))
                .collect(),
            strips: levels.strips,
            buses: levels.buses,
            now,
        }
    }
}

/// The `RenderSinks::sample` closure over `source`.
#[must_use]
pub fn sampler(source: LevelSource) -> Box<dyn FnMut() -> LiveValues + Send> {
    let mut sampler = Sampler::new(source);
    let origin = Instant::now();
    Box::new(move || sampler.sample_at(origin.elapsed().as_secs_f64(), SystemTime::now()))
}

#[cfg(test)]
mod tests;
