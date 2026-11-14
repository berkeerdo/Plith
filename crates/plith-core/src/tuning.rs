//! Spring tunings for every motion, from the research table (unit mass).

/// Stiffness and damping of a unit-mass spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    /// Stiffness `k`.
    pub stiffness: f64,
    /// Damping coefficient `c`.
    pub damping: f64,
}

impl Tuning {
    /// Creates a tuning from stiffness `k` and damping `c`.
    #[must_use]
    pub const fn new(stiffness: f64, damping: f64) -> Self {
        Self { stiffness, damping }
    }

    /// Damping ratio `c / (2 sqrt(k))`; 1.0 is critical, below 1.0 overshoots.
    #[must_use]
    pub fn zeta(&self) -> f64 {
        self.damping / (2.0 * self.stiffness.sqrt())
    }
}

/// Island width while growing (zeta 0.73).
pub const GROW_W: Tuning = Tuning::new(500.0, 32.5);
/// Island height while growing (zeta 0.73).
pub const GROW_H: Tuning = Tuning::new(250.0, 23.0);
/// Island width while shrinking (zeta 0.90).
pub const SHRINK_W: Tuning = Tuning::new(400.0, 36.0);
/// Island height while shrinking (zeta 0.91).
pub const SHRINK_H: Tuning = Tuning::new(560.0, 43.0);
/// Corner radius while growing (critical).
pub const RADIUS_GROW: Tuning = Tuning::new(1200.0, 69.3);
/// Corner radius while shrinking (critical).
pub const RADIUS_SHRINK: Tuning = Tuning::new(150.0, 24.5);
/// Page turn slide offset (zeta 0.90).
pub const PAGE_OFFSET: Tuning = Tuning::new(380.0, 35.1);
/// Opacity fades (critical).
pub const OPACITY: Tuning = Tuning::new(700.0, 52.9);

#[cfg(test)]
mod tests;
