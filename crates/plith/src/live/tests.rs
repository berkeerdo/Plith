use super::*;
use crate::services::Levels;
use std::time::Duration;

fn fixed(strips: usize) -> LevelSource {
    Box::new(move || {
        Some(Levels {
            strips: vec![[1.0, 0.5]; strips],
            buses: vec![[0.25, 0.25]; 2],
        })
    })
}

#[test]
fn sample_copies_levels_and_keeps_one_trace_per_strip() {
    let mut sampler = Sampler::new(fixed(3));
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(9);
    let mut values = sampler.sample_at(0.0, now);
    for i in 1..=30 {
        values = sampler.sample_at(f64::from(i) / 30.0, now);
    }
    assert_eq!(values.strips, vec![[1.0, 0.5]; 3]);
    assert_eq!(values.buses, vec![[0.25, 0.25]; 2]);
    assert_eq!(values.now, now);
    assert_eq!(values.traces.len(), 3);
    assert_eq!(values.traces[0].0.len(), 31);
    assert!(values.trace(2).iter().all(|v| (v - 1.0).abs() < 1e-6));
}

#[test]
fn no_mixer_samples_only_the_time() {
    let mut sampler = Sampler::new(Box::new(|| None));
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(3);
    let values = sampler.sample_at(0.5, now);
    assert!(values.strips.is_empty() && values.buses.is_empty() && values.traces.is_empty());
    assert_eq!(values.now, now);
}
