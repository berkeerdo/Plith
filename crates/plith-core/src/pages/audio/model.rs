//! The rules behind the audio view: rows, route states, density, signal activity.

use super::{
    AudioFacts, AudioView, Footer, MAX_OUTPUT_SLOTS, OutputRow, OutputValue, PickMode, Route,
    RouteState, SourceRow, UNUSED_NAME, VmFacts, row_pitch,
};

/// Peak level above which a strip counts as carrying signal.
const SIGNAL_PEAK: f32 = 0.01;
/// How long signal is held after the last loud frame, in seconds.
const HOLD_S: f64 = 2.0;
/// Captions of the virtual strips, in order.
const VIRTUAL_SUBS: [&str; 3] = ["VAIO", "AUX", "VAIO3"];

/// Per-strip signal memory with a two second hold.
#[derive(Debug, Clone, Default)]
pub struct Activity {
    last: Vec<Option<f64>>,
}

impl Activity {
    /// Records a frame of per-strip `[left, right]` peaks taken at `now_s` seconds.
    pub fn observe(&mut self, strips: &[[f32; 2]], now_s: f64) {
        if self.last.len() < strips.len() {
            self.last.resize(strips.len(), None);
        }
        for (slot, peaks) in self.last.iter_mut().zip(strips) {
            if peaks[0].max(peaks[1]) > SIGNAL_PEAK {
                *slot = Some(now_s);
            }
        }
    }

    /// Whether strip `s` had signal within the hold time before `now_s`.
    #[must_use]
    pub fn has_signal(&self, s: usize, now_s: f64) -> bool {
        matches!(self.last.get(s), Some(Some(t)) if now_s - t <= HOLD_S)
    }
}

type Parts = (Vec<SourceRow>, Vec<OutputRow>, Vec<Route>, Footer);

pub(super) fn build(
    facts: &AudioFacts,
    activity: &Activity,
    mode: PickMode,
    now_s: f64,
) -> AudioView {
    let (mut sources, mut outputs, mut routes, base_footer) = match &facts.vm {
        Some(vm) => voicemeeter(vm, activity, now_s),
        None => windows(facts),
    };
    for (o, row) in outputs.iter_mut().enumerate() {
        row.used = routes.iter().any(|r| r.output == o);
    }
    let picked = match mode {
        PickMode::Picked(s) if facts.vm.is_some() && s < sources.len() => Some(s),
        _ => None,
    };
    if let Some(s) = picked {
        sources[s].picked = true;
        for o in 0..outputs.len() {
            if !routes.iter().any(|r| r.source == s && r.output == o) {
                routes.push(Route {
                    source: s,
                    output: o,
                    state: RouteState::Candidate,
                });
            }
        }
    }
    routes.retain(|r| r.state != RouteState::Idle || Some(r.source) == picked);
    let mode = picked.map_or(PickMode::None, PickMode::Picked);
    for (o, row) in outputs.iter_mut().enumerate() {
        row.live = routes
            .iter()
            .any(|r| r.output == o && r.state == RouteState::Live);
    }
    let footer = match (base_footer, picked) {
        (Footer::NoVoicemeeter, _) => Footer::NoVoicemeeter,
        (_, Some(_)) => Footer::Picking,
        _ => Footer::Normal,
    };
    let row_pitch = row_pitch(sources.len().max(outputs.len().min(MAX_OUTPUT_SLOTS)));
    AudioView {
        sources,
        outputs,
        routes,
        mode,
        footer,
        row_pitch,
    }
}

fn voicemeeter(vm: &VmFacts, activity: &Activity, now_s: f64) -> Parts {
    let sources = vm
        .strips
        .iter()
        .enumerate()
        .map(|(i, s)| SourceRow {
            name: if s.label.is_empty() {
                format!("Kaynak {}", i + 1)
            } else {
                s.label.clone()
            },
            sub: strip_sub(vm, i),
            muted: s.muted,
            picked: false,
        })
        .collect();
    let outputs = (0..vm.buses.len()).map(|b| bus_row(vm, b)).collect();
    let mut routes = Vec::new();
    for (s, strip) in vm.strips.iter().enumerate() {
        for (b, bus) in vm.buses.iter().enumerate() {
            if !strip.routes.get(b).copied().unwrap_or(false) {
                continue;
            }
            let state = if strip.muted || bus.muted {
                RouteState::Cut
            } else if !bus.muted && activity.has_signal(s, now_s) {
                RouteState::Live
            } else {
                RouteState::Idle
            };
            routes.push(Route {
                source: s,
                output: b,
                state,
            });
        }
    }
    (sources, outputs, routes, Footer::Normal)
}

fn strip_sub(vm: &VmFacts, i: usize) -> String {
    if i < vm.hardware_strips {
        format!("HW {}", i + 1)
    } else {
        VIRTUAL_SUBS
            .get(i - vm.hardware_strips)
            .copied()
            .unwrap_or("")
            .to_string()
    }
}

fn bus_row(vm: &VmFacts, b: usize) -> OutputRow {
    let bus = &vm.buses[b];
    let tag = if b < vm.hardware_buses {
        format!("A{}", b + 1)
    } else {
        format!("B{}", b - vm.hardware_buses + 1)
    };
    let used = vm
        .strips
        .iter()
        .any(|s| s.routes.get(b).copied().unwrap_or(false));
    // A bus is named when its label is set and is not just its tag.
    let named = !bus.label.is_empty() && bus.label != tag;
    // The mock shows no value on an unused, unnamed bus.
    let unnamed = !named && !used;
    let name = if named {
        bus.label.clone()
    } else if used {
        tag.clone()
    } else {
        UNUSED_NAME.to_string()
    };
    let value = if unnamed {
        OutputValue::None
    } else {
        OutputValue::Db(bus.gain_db)
    };
    OutputRow {
        tag: Some(tag),
        name,
        value,
        live: false,
        muted: bus.muted,
        id: None,
        used: false,
        unnamed,
    }
}

fn windows(facts: &AudioFacts) -> Parts {
    let muted = matches!(facts.attached, Some((_, true)));
    let sources = vec![SourceRow {
        name: "Windows".to_string(),
        sub: crate::pages::copy::DEFAULT_OUTPUT.to_string(),
        muted,
        picked: false,
    }];
    let default_idx = facts
        .default_id
        .as_ref()
        .and_then(|id| facts.endpoints.iter().position(|e| &e.id == id));
    let outputs = facts
        .endpoints
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let value = match (Some(i) == default_idx, facts.attached) {
                (true, Some((v, _))) => OutputValue::Percent(percent(v)),
                _ => OutputValue::None,
            };
            OutputRow {
                tag: None,
                name: e.name.clone(),
                value,
                live: false,
                muted: Some(i) == default_idx && muted,
                id: Some(e.id.clone()),
                used: false,
                unnamed: false,
            }
        })
        .collect();
    let routes = default_idx
        .map(|o| {
            let state = if muted {
                RouteState::Cut
            } else {
                RouteState::Live
            };
            vec![Route {
                source: 0,
                output: o,
                state,
            }]
        })
        .unwrap_or_default();
    (sources, outputs, routes, Footer::NoVoicemeeter)
}

/// A 0..1 volume as a whole percent, clamped to 0..=100.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(v: f32) -> u8 {
    // The value is clamped to 0..=100 first, so the cast cannot truncate or lose a sign.
    (v * 100.0).round().clamp(0.0, 100.0) as u8
}
