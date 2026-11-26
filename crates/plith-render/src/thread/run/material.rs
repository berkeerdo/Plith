//! The material timeline in the loop: the engine starts the open and collapse sequences when
//! the scene's `expanded` flips, fills each frame's layers from the morph progress and keeps a
//! motion running until the ripples are done. A plain theme paints `MaterialLayers::OPEN` and
//! settles on its springs alone.

use super::super::Paint;
use super::engine::Engine;
use crate::clock::{ClockError, FrameSource};
use plith_core::clock_choice::ClockKind;
use plith_core::material::art::ThemePaint;
use plith_core::material::{MaterialLayers, MaterialMotion, morph_progress};
use plith_core::scene::Scene;
use plith_core::shapes::{PAGES, REST};

/// The timeline at rest for `scene`: open when it is expanded, collapsed otherwise.
pub(super) fn material_at_rest(scene: Scene) -> MaterialMotion {
    if scene.expanded {
        MaterialMotion::open()
    } else {
        MaterialMotion::new()
    }
}

/// The height progress is measured against for `scene` at rest: its target when expanded, the
/// first page otherwise (any open height works while collapsed: the shape is at rest height).
pub(super) fn open_height_of(scene: Scene) -> f64 {
    if scene.expanded {
        scene.target().height
    } else {
        PAGES[0].height
    }
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    fn now_ms(&self) -> f64 {
        (self.now)() * 1000.0
    }

    /// The morph progress of the drawn shape: 0 at rest height, 1 at the open height.
    fn morph(&self) -> f64 {
        morph_progress(self.island.shape().height, REST.height, self.open_height)
    }

    /// Starts the open or collapse sequence when `next` flips `expanded`; call it before the
    /// scene is replaced. An open measures against its target; a page switch while open keeps
    /// the smaller of the two heights, so the progress never drops below 1 on a taller page; a
    /// collapse measures from the open height or the drawn height, whichever is taller.
    pub(super) fn aim_material(&mut self, next: Scene) {
        let now = self.now_ms();
        match (self.scene.expanded, next.expanded) {
            (false, true) => {
                self.open_height = next.target().height;
                let progress = self.morph();
                self.material.expand(now, progress);
            }
            (true, true) => self.open_height = self.open_height.min(next.target().height),
            (true, false) => {
                self.open_height = self.open_height.max(self.island.shape().height);
                self.material.collapse(now);
            }
            (false, false) => {}
        }
    }

    /// Puts the timeline at rest for `scene` (a snap: no motion).
    pub(super) fn snap_material(&mut self, scene: Scene) {
        self.material = material_at_rest(scene);
        self.open_height = open_height_of(scene);
    }

    /// The layers this frame paints: the timeline's for a material theme, `OPEN` otherwise.
    pub(super) fn material_layers(&self) -> MaterialLayers {
        if self.has_material {
            self.material.layers(self.now_ms(), self.morph())
        } else {
            MaterialLayers::OPEN
        }
    }

    /// True when the material has nothing left to animate (always for a plain theme).
    pub(super) fn material_settled(&self) -> bool {
        !self.has_material || self.material.is_settled(self.now_ms(), self.morph())
    }

    /// Notes the theme the painter is about to draw: its id for the motion line and whether it
    /// has a material.
    pub(super) fn note_theme(&mut self, paint: &ThemePaint) {
        self.theme.clone_from(&paint.drawn);
        self.has_material = paint.material.is_some();
    }
}
