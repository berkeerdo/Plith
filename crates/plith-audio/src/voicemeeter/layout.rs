//! Parameter names and fallback labels.

use super::Rail;

/// Builds a Remote API parameter name, e.g. `Strip[2].Gain`.
#[must_use]
pub fn param_name(rail: Rail, index: usize, param: &str) -> String {
    let prefix = match rail {
        Rail::Strip => "Strip",
        Rail::Bus => "Bus",
    };
    format!("{prefix}[{index}].{param}")
}

/// Label used when the engine reports an empty one: `Strip 1`, or `A1..A5`, `B1..B3` for buses.
#[must_use]
pub fn fallback_label(rail: Rail, index: usize) -> String {
    match rail {
        Rail::Strip => format!("Strip {}", index + 1),
        Rail::Bus if index < 5 => format!("A{}", index + 1),
        Rail::Bus => format!("B{}", index - 4),
    }
}
