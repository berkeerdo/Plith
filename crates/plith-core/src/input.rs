//! Pointer input on the island: which page a click lands on, what it does, and the delayed
//! collapse after the pointer leaves.

use crate::page::{PageLayer, PageLayers};
use crate::pages::audio::{PickMode, VmFacts};
use crate::pages::{Action, HitTarget, PAGE_COUNT, hit};

/// Seconds the pointer must stay outside the island before it collapses.
pub const COLLAPSE_DELAY_S: f64 = 0.4;

/// What a click inside the drawn island does.
#[derive(Debug, Clone, PartialEq)]
pub enum ClickResult {
    /// The island is collapsed: open it.
    Expand,
    /// A page target was hit.
    Action(Action),
    /// Empty page space: nothing happens (the island stays open).
    Nothing,
}

/// The page layer a click reaches: the incoming layer (drawn last) once its opacity is at
/// least 0.5, else the outgoing one. `None` when no layer is drawn.
#[must_use]
pub fn page_on_screen(layers: &PageLayers) -> Option<PageLayer> {
    let mut drawn = layers.iter();
    let first = *drawn.next()?;
    match drawn.next() {
        Some(incoming) if incoming.opacity >= 0.5 => Some(*incoming),
        _ => Some(first),
    }
}

/// Routes a click at `(x, y)` island DIP. Collapsed, any click expands. Expanded, the targets
/// of the page on screen are tested at `(x - offset, y)`.
#[must_use]
pub fn route_click(
    expanded: bool,
    layers: &PageLayers,
    targets: &[Vec<HitTarget>; PAGE_COUNT],
    x: f64,
    y: f64,
) -> ClickResult {
    if !expanded {
        return ClickResult::Expand;
    }
    let Some(layer) = page_on_screen(layers) else {
        return ClickResult::Nothing;
    };
    let Some(page) = targets.get(layer.page) else {
        return ClickResult::Nothing;
    };
    hit(page, x - layer.offset, y).map_or(ClickResult::Nothing, ClickResult::Action)
}

/// The pick mode after a press on source `source`: the same source again cancels, any other
/// source becomes the picked one.
#[must_use]
pub fn toggle_pick(current: PickMode, source: usize) -> PickMode {
    if current == PickMode::Picked(source) {
        PickMode::None
    } else {
        PickMode::Picked(source)
    }
}

/// `pick` when its source is one of `sources` rows, else no pick (the source went away).
#[must_use]
pub fn pick_within(pick: PickMode, sources: usize) -> PickMode {
    match pick {
        PickMode::Picked(source) if source < sources => pick,
        _ => PickMode::None,
    }
}

/// The layout of the mixer a pick was made in. Voicemeeter editions differ in their strip and
/// bus counts, so a pick from one edition must not route on another (indices would name a
/// different strip/bus pair).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerShape {
    /// All strips.
    pub strips: usize,
    /// All buses.
    pub buses: usize,
    /// Hardware strips.
    pub hardware_strips: usize,
    /// Hardware buses.
    pub hardware_buses: usize,
}

impl MixerShape {
    /// The shape of `vm`, or `None` without a mixer.
    #[must_use]
    pub fn of(vm: Option<&VmFacts>) -> Option<Self> {
        vm.map(|vm| Self {
            strips: vm.strips.len(),
            buses: vm.buses.len(),
            hardware_strips: vm.hardware_strips,
            hardware_buses: vm.hardware_buses,
        })
    }
}

/// Whether a route picked in mixer `picked` may be written to mixer `now`: both exist and match.
#[must_use]
pub fn route_allowed(picked: Option<MixerShape>, now: Option<MixerShape>) -> bool {
    picked.is_some() && picked == now
}

/// The delayed collapse: armed when the pointer leaves, disarmed when it comes back.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CollapseTimer {
    /// When the pointer left, in seconds of the caller's clock; `None` while inside.
    pub leave_at: Option<f64>,
}

impl CollapseTimer {
    /// The pointer left the island at `now_s`.
    pub fn leave(&mut self, now_s: f64) {
        if self.leave_at.is_none() {
            self.leave_at = Some(now_s);
        }
    }

    /// The pointer is back inside.
    pub fn enter(&mut self) {
        self.leave_at = None;
    }

    /// Seconds left until [`due`](Self::due) at `now_s`; `None` while inside.
    #[must_use]
    pub fn remaining(&self, now_s: f64) -> Option<f64> {
        self.leave_at
            .map(|left| (COLLAPSE_DELAY_S - (now_s - left)).max(0.0))
    }

    /// True when the pointer has been out for [`COLLAPSE_DELAY_S`] by `now_s`.
    #[must_use]
    pub fn due(&self, now_s: f64) -> bool {
        self.leave_at
            .is_some_and(|left| now_s - left >= COLLAPSE_DELAY_S - 1e-9)
    }
}

#[cfg(test)]
mod tests;
