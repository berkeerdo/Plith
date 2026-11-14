//! Three-axis spring morph of the island: width, height and corner radius.

use crate::geometry::IslandShape;
use crate::spring::Spring;
use crate::tuning::{GROW_H, GROW_W, RADIUS_GROW, RADIUS_SHRINK, SHRINK_H, SHRINK_W, Tuning};

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
}

impl IslandMotion {
    /// Creates a settled motion at `rest`; each axis floor starts at its rest value.
    #[must_use]
    pub fn new(rest: IslandShape) -> Self {
        Self {
            width: Axis::new(rest.width, GROW_W),
            height: Axis::new(rest.height, GROW_H),
            radius: Axis::new(rest.radius, RADIUS_GROW),
        }
    }

    /// Aims every axis at `target`, retuning each from its own direction.
    pub fn retarget(&mut self, target: IslandShape) {
        self.width.retarget(target.width, GROW_W, SHRINK_W);
        self.height.retarget(target.height, GROW_H, SHRINK_H);
        self.radius
            .retarget(target.radius, RADIUS_GROW, RADIUS_SHRINK);
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
