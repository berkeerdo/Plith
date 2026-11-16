//! Parameter names and fallback labels.

use super::{Rail, VmKind};

/// Builds a Remote API parameter name, e.g. `Strip[2].Gain`.
#[must_use]
pub fn param_name(rail: Rail, index: usize, param: &str) -> String {
    let prefix = match rail {
        Rail::Strip => "Strip",
        Rail::Bus => "Bus",
    };
    format!("{prefix}[{index}].{param}")
}

/// Label used when the engine reports an empty one: `Strip 1`, or for buses `A1..` then `B1..`
/// with the split depending on the edition (Standard A1, B1; Banana A1..A3, B1,B2; Potato
/// A1..A5, B1..B3).
#[must_use]
pub fn fallback_label(kind: VmKind, rail: Rail, index: usize) -> String {
    match rail {
        Rail::Strip => format!("Strip {}", index + 1),
        Rail::Bus => {
            let hardware_outputs = kind.hardware_buses();
            if index < hardware_outputs {
                format!("A{}", index + 1)
            } else {
                format!("B{}", index - hardware_outputs + 1)
            }
        }
    }
}

/// Strip parameter suffixes for the bus assignments, in bus order: `A1..` then `B1..`.
const ROUTE_PARAMS: [&str; 8] = ["A1", "A2", "A3", "A4", "A5", "B1", "B2", "B3"];

/// Remote API suffix of the strip parameter that routes a strip to output `bus` (`Strip[i].A1`),
/// split per edition exactly as [`fallback_label`]. `None` when the edition has no such bus.
#[must_use]
pub fn route_param(kind: VmKind, bus: usize) -> Option<&'static str> {
    if bus >= kind.bus_count() {
        return None;
    }
    let hardware_outputs = kind.hardware_buses();
    let slot = if bus < hardware_outputs {
        bus
    } else {
        5 + bus - hardware_outputs
    };
    ROUTE_PARAMS.get(slot).copied()
}

/// Reads every bus assignment of strip `index` through `get` (the float getter). A value of at
/// least 0.5 is on; a failed read counts as off, like the mute flag.
pub fn read_routes(
    kind: VmKind,
    index: usize,
    mut get: impl FnMut(&str) -> Option<f32>,
) -> Vec<bool> {
    (0..kind.bus_count())
        .map(|bus| {
            route_param(kind, bus)
                .and_then(|suffix| get(&param_name(Rail::Strip, index, suffix)))
                .is_some_and(|v| v >= 0.5)
        })
        .collect()
}
