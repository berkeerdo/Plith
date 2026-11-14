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
/// with the split depending on the edition (Standard A1,A2; Banana A1..A3, B1,B2; Potato
/// A1..A5, B1..B3).
#[must_use]
pub fn fallback_label(kind: VmKind, rail: Rail, index: usize) -> String {
    match rail {
        Rail::Strip => format!("Strip {}", index + 1),
        Rail::Bus => {
            let hardware_outputs = match kind {
                VmKind::Standard => 2,
                VmKind::Banana => 3,
                VmKind::Potato => 5,
            };
            if index < hardware_outputs {
                format!("A{}", index + 1)
            } else {
                format!("B{}", index - hardware_outputs + 1)
            }
        }
    }
}
