//! Island geometry: a rounded rectangle given by size and corner radius.

/// Size and corner radius of the island, in DIP.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IslandShape {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
    /// Corner radius.
    pub radius: f64,
}

/// Clamps `v` into `[r, max - r]`: the nearest point of the corner-circle centre range.
fn centre_coord(v: f64, max: f64, r: f64) -> f64 {
    v.clamp(r, max - r)
}

impl IslandShape {
    /// Area of the bounding rectangle.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// True when the point (relative to the island's top-left) is inside the rounded rectangle.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        if x < 0.0 || y < 0.0 || x > self.width || y > self.height {
            return false;
        }
        let r = self
            .radius
            .min(self.width / 2.0)
            .min(self.height / 2.0)
            .max(0.0);
        let dx = x - centre_coord(x, self.width, r);
        let dy = y - centre_coord(y, self.height, r);
        dx * dx + dy * dy <= r * r
    }
}

#[cfg(test)]
mod tests;
