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

/// Every tuning that shapes a motion, so presets can be compared side by side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TuningSet {
    /// Preset name as typed after `--tune`.
    pub name: &'static str,
    /// Island width while growing.
    pub grow_w: Tuning,
    /// Island height while growing.
    pub grow_h: Tuning,
    /// Island width while shrinking.
    pub shrink_w: Tuning,
    /// Island height while shrinking.
    pub shrink_h: Tuning,
    /// Corner radius while growing.
    pub radius_grow: Tuning,
    /// Corner radius while shrinking.
    pub radius_shrink: Tuning,
    /// Width while growing from one card to another (neither end is the rest pill).
    pub card_grow_w: Tuning,
    /// Height while growing from one card to another.
    pub card_grow_h: Tuning,
    /// Page turn slide offset.
    pub page_offset: Tuning,
    /// Opacity fades (page layers and content).
    pub opacity: Tuning,
}

/// Today's tunings (the R1 log was measured with these).
pub const R1: TuningSet = TuningSet {
    name: "r1",
    grow_w: GROW_W,
    grow_h: GROW_H,
    shrink_w: SHRINK_W,
    shrink_h: SHRINK_H,
    radius_grow: RADIUS_GROW,
    radius_shrink: RADIUS_SHRINK,
    card_grow_w: GROW_W,
    card_grow_h: GROW_H,
    page_offset: PAGE_OFFSET,
    opacity: OPACITY,
};

/// R1 with a firmer height spring (zeta 0.81) and firmer card-to-card growth (zeta 0.89).
pub const BRISK: TuningSet = TuningSet {
    name: "brisk",
    grow_h: Tuning::new(320.0, 29.0),
    card_grow_w: Tuning::new(500.0, 40.0),
    card_grow_h: Tuning::new(500.0, 40.0),
    ..R1
};

impl TuningSet {
    /// The preset called `name` (`r1` or `brisk`).
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "r1" => Some(R1),
            "brisk" => Some(BRISK),
            _ => None,
        }
    }
}

impl Default for TuningSet {
    fn default() -> Self {
        BRISK
    }
}

#[cfg(test)]
mod tests;
