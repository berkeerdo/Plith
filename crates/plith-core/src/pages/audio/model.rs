//! The rules behind the audio view: rows, route states and density. Signal comes from the
//! mixer's debounced flags (`VmFacts::signal`), which change only on an edge.

use crate::pages::derive;

use super::{
    AudioFacts, AudioView, Footer, MAX_OUTPUT_SLOTS, OutputRow, OutputValue, PickMode, Route,
    RouteState, SourceRow, UNUSED_NAME, VmFacts, row_pitch,
};

/// Captions of the virtual strips, in order.
const VIRTUAL_SUBS: [&str; 3] = ["VAIO", "AUX", "VAIO3"];

type Parts = (Vec<SourceRow>, Vec<OutputRow>, Vec<Route>, Footer);

pub(super) fn build(facts: &AudioFacts, mode: PickMode) -> AudioView {
    let (mut sources, mut outputs, mut routes, base_footer) = match &facts.vm {
        Some(vm) => voicemeeter(vm),
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

fn voicemeeter(vm: &VmFacts) -> Parts {
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
            } else if vm.signal.get(s).copied().unwrap_or(false) {
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
    let tag = derive::bus_tag(vm, b);
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
                (true, Some((v, _))) => OutputValue::Percent(derive::percent(v)),
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
