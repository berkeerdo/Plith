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

    /// The corner radius as drawn: the declared radius limited to half the width and height.
    #[must_use]
    pub fn drawn_radius(&self) -> f64 {
        self.radius
            .min(self.width / 2.0)
            .min(self.height / 2.0)
            .max(0.0)
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

/// A rounded rectangle that contains both `a` and `b` when each is centred horizontally and
/// anchored at the top, as the island is: the larger width and height with the smaller drawn
/// radius. Used for the hit region, which must also cover the frame still on screen.
#[must_use]
pub fn cover(a: IslandShape, b: IslandShape) -> IslandShape {
    IslandShape {
        width: a.width.max(b.width),
        height: a.height.max(b.height),
        radius: a.drawn_radius().min(b.drawn_radius()),
    }
}

#[cfg(test)]
mod tests;
