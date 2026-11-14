//! What a viewer can see of a motion: the drawn state and when it stops differing from rest.

use crate::geometry::IslandShape;
use crate::page::{PageLayer, PageLayers};

/// Width, height, radius and page offset changes below this many DIP are not visible.
const POSITION_EPS: f64 = 0.5;
/// Opacity changes below half an 8-bit step are not visible.
const OPACITY_EPS: f64 = 0.5 / 255.0;

/// Everything one frame draws: the island, the content fade and the page layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnState {
    /// The drawn shape (radius already clamped to the size).
    pub shape: IslandShape,
    /// Content opacity from 0 to 1.
    pub content: f64,
    /// Visible page layers.
    pub pages: PageLayers,
}

impl DrawnState {
    /// The state a frame settles to for `scene_target`: content 1 when `expanded` and 0 when
    /// not, and one incoming layer of `page` at offset 0 and opacity 1.
    #[must_use]
    pub fn at_rest(scene_target: IslandShape, expanded: bool, page: usize) -> Self {
        Self {
            shape: scene_target,
            content: if expanded { 1.0 } else { 0.0 },
            pages: PageLayers::of(&[PageLayer {
                page,
                offset: 0.0,
                opacity: 1.0,
            }]),
        }
    }
}

/// True when a viewer could tell `a` from `b`: width, height, drawn radius or a page offset
/// more than 0.5 DIP apart, or any opacity more than half an 8-bit step apart. A layer present
/// in one state and absent in the other differs when its opacity is above that step. Pages are
/// drawn multiplied by the content opacity, so while both states have content under that step
/// the page layers are invisible and are not compared.
#[must_use]
pub fn visibly_differs(a: &DrawnState, b: &DrawnState) -> bool {
    let pages_hidden = a.content < OPACITY_EPS && b.content < OPACITY_EPS;
    shape_differs(&a.shape, &b.shape)
        || (a.content - b.content).abs() > OPACITY_EPS
        || (!pages_hidden && layers_differ(&a.pages, &b.pages))
}

fn shape_differs(a: &IslandShape, b: &IslandShape) -> bool {
    (a.width - b.width).abs() > POSITION_EPS
        || (a.height - b.height).abs() > POSITION_EPS
        || (a.radius - b.radius).abs() > POSITION_EPS
}

fn layers_differ(a: &PageLayers, b: &PageLayers) -> bool {
    one_sided(a, b) || one_sided(b, a)
}

/// Checks each layer of `from` against the layer of `other` that shows the same page.
fn one_sided(from: &PageLayers, other: &PageLayers) -> bool {
    from.iter().any(
        |layer| match other.iter().find(|candidate| candidate.page == layer.page) {
            Some(twin) => {
                (layer.offset - twin.offset).abs() > POSITION_EPS
                    || (layer.opacity - twin.opacity).abs() > OPACITY_EPS
            }
            None => layer.opacity > OPACITY_EPS,
        },
    )
}

#[cfg(test)]
mod tests;
