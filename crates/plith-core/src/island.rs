//! Three-axis spring morph of the island: width, height and corner radius.

use crate::geometry::IslandShape;
use crate::shapes::REST;
use crate::spring::Spring;
use crate::tuning::{Tuning, TuningSet};

/// One spring plus the lowest value it may ever show.
#[derive(Debug, Clone)]
struct Axis {
    spring: Spring,
    floor: f64,
}

impl Axis {
    fn new(value: f64, tuning: Tuning) -> Self {
        Self {
            spring: Spring::new(value, tuning),
            floor: value,
        }
    }

    /// Retunes from this axis's own direction; a shrinking axis lands its floor on the target.
    fn retarget(&mut self, target: f64, grow: Tuning, shrink: Tuning) {
        if target < self.spring.value() {
            self.floor = target;
            self.spring.retarget(target, shrink);
        } else {
            self.spring.retarget(target, grow);
        }
    }

    fn step(&mut self, dt: f64) -> bool {
        let settled = self.spring.step(dt);
        self.spring.clamp_min(self.floor);
        settled
    }
}

/// The island's animated shape.
#[derive(Debug, Clone)]
pub struct IslandMotion {
    width: Axis,
    height: Axis,
    radius: Axis,
    set: TuningSet,
    target: IslandShape,
}

impl IslandMotion {
    /// Creates a settled motion at `rest` under `set`; each axis floor starts at its rest value.
    #[must_use]
    pub fn new(rest: IslandShape, set: TuningSet) -> Self {
        Self {
            width: Axis::new(rest.width, set.grow_w),
            height: Axis::new(rest.height, set.grow_h),
            radius: Axis::new(rest.radius, set.radius_grow),
            set,
            target: rest,
        }
    }

    /// Aims every axis at `target`, retuning each from its own direction. A growing width or
    /// height uses the card-to-card tunings when neither the old nor the new target is `REST`.
    pub fn retarget(&mut self, target: IslandShape) {
        let set = self.set;
        let card = target != REST && self.target != REST;
        let (grow_w, grow_h) = if card {
            (set.card_grow_w, set.card_grow_h)
        } else {
            (set.grow_w, set.grow_h)
        };
        self.width.retarget(target.width, grow_w, set.shrink_w);
        self.height.retarget(target.height, grow_h, set.shrink_h);
        self.radius
            .retarget(target.radius, set.radius_grow, set.radius_shrink);
        self.target = target;
    }

    /// Advances all axes by `dt` seconds; true when every axis is settled.
    pub fn step(&mut self, dt: f64) -> bool {
        let w = self.width.step(dt);
        let h = self.height.step(dt);
        let r = self.radius.step(dt);
        w && h && r
    }

    /// Current shape, with the radius clamped to half the width and height.
    #[must_use]
    pub fn shape(&self) -> IslandShape {
        let width = self.width.spring.value();
        let height = self.height.spring.value();
        let radius = self
            .radius
            .spring
            .value()
            .min(height / 2.0)
            .min(width / 2.0)
            .max(0.0);
        IslandShape {
            width,
            height,
            radius,
        }
    }

    /// True when every axis is settled.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.width.spring.is_settled()
            && self.height.spring.is_settled()
            && self.radius.spring.is_settled()
    }
}

#[cfg(test)]
mod tests;
