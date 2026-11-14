//! A damped spring integrated with a fixed 1/240 s sub-step (semi-implicit Euler).

use crate::tuning::Tuning;

/// Fixed integration sub-step in seconds.
const SUBSTEP: f64 = 1.0 / 240.0;
/// Largest `dt` accepted by one `step` call, in seconds.
const MAX_DT: f64 = 0.1;
/// Slack so that float rounding never loses a whole sub-step.
const SLACK: f64 = 1e-12;

/// One animated scalar chasing a target.
#[derive(Debug, Clone)]
pub struct Spring {
    value: f64,
    velocity: f64,
    target: f64,
    tuning: Tuning,
    value_eps: f64,
    velocity_eps: f64,
    leftover: f64,
}

impl Spring {
    /// Creates a settled spring at `value` (settle thresholds 0.05 and 0.2).
    #[must_use]
    pub fn new(value: f64, tuning: Tuning) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            tuning,
            value_eps: 0.05,
            velocity_eps: 0.2,
            leftover: 0.0,
        }
    }

    /// Replaces the settle thresholds.
    #[must_use]
    pub fn with_eps(mut self, value_eps: f64, velocity_eps: f64) -> Self {
        self.value_eps = value_eps;
        self.velocity_eps = velocity_eps;
        self
    }

    /// Aims at a new target with a new tuning, keeping value and velocity.
    pub fn retarget(&mut self, target: f64, tuning: Tuning) {
        self.target = target;
        self.tuning = tuning;
    }

    /// Advances by `dt` seconds (clamped to 0..=0.1); true when settled.
    pub fn step(&mut self, dt: f64) -> bool {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_DT)
        } else {
            0.0
        };
        self.leftover += dt;
        while self.leftover >= SUBSTEP - SLACK {
            self.leftover = (self.leftover - SUBSTEP).max(0.0);
            if self.is_settled() {
                break;
            }
            let force = -self.tuning.stiffness * (self.value - self.target)
                - self.tuning.damping * self.velocity;
            self.velocity += force * SUBSTEP;
            self.value += self.velocity * SUBSTEP;
        }
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return true;
        }
        false
    }

    /// Raises the value to `floor` and drops negative velocity when below it.
    pub fn clamp_min(&mut self, floor: f64) {
        if self.value < floor {
            self.value = floor;
            if self.velocity < 0.0 {
                self.velocity = 0.0;
            }
        }
    }

    /// Current value.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Current velocity.
    #[must_use]
    pub fn velocity(&self) -> f64 {
        self.velocity
    }

    /// Current target.
    #[must_use]
    pub fn target(&self) -> f64 {
        self.target
    }

    /// True when both distance and speed are under their thresholds.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < self.value_eps && self.velocity.abs() < self.velocity_eps
    }

    #[cfg(test)]
    fn set_velocity_for_test(&mut self, velocity: f64) {
        self.velocity = velocity;
    }
}

#[cfg(test)]
mod tests;
