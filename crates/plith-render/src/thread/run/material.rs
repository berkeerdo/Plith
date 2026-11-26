//! The material timeline in the loop: the engine starts the open and collapse sequences when
//! the scene's `expanded` flips, advances the timeline and fills each frame's layers from the
//! morph progress, and keeps a motion running until the ripples are done. A plain theme paints
//! `MaterialLayers::OPEN` and settles on its springs alone.

use super::super::Paint;
use super::engine::Engine;
use crate::clock::{ClockError, FrameSource};
use plith_core::clock_choice::ClockKind;
use plith_core::material::art::ThemePaint;
use plith_core::material::{MaterialLayers, MaterialMotion, MorphLine};
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

/// The progress line for `scene` at rest: 1 at its target when expanded, 0 at rest height
/// otherwise (toward the first page, any open height works while collapsed).
pub(super) fn morph_at_rest(scene: Scene) -> MorphLine {
    let open = if scene.expanded {
        scene.target().height
    } else {
        PAGES[0].height
    };
    MorphLine::open(REST.height, open)
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

    /// The morph progress of the drawn shape on the current line.
    fn morph_now(&self) -> f64 {
        self.morph.progress(self.island.shape().height)
    }

    /// Starts the open or collapse sequence when `next` flips `expanded`; call it before the
    /// scene is replaced. Every retarget starts a new progress line at the progress the island
    /// has now, so nothing jumps: an open or a page switch runs to 1 at the new target, a
    /// collapse runs to 0 at rest height.
    pub(super) fn aim_material(&mut self, next: Scene) {
        let now = self.now_ms();
        let height = self.island.shape().height;
        match (self.scene.expanded, next.expanded) {
            (false, true) => {
                self.morph = self.morph.toward(height, next.target().height);
                let progress = self.morph_now();
                self.material.expand(now, progress);
            }
            (true, true) => self.morph = self.morph.toward(height, next.target().height),
            (true, false) => {
                self.morph = self.morph.collapse(height, REST.height);
                self.material.collapse(now);
            }
            (false, false) => {}
        }
    }

    /// Puts the timeline at rest for `scene` (a snap: no motion).
    pub(super) fn snap_material(&mut self, scene: Scene) {
        self.material = material_at_rest(scene);
        self.morph = morph_at_rest(scene);
    }

    /// Records this frame in the timeline (latches the ripple presses); before the layers.
    pub(super) fn advance_material(&mut self) {
        let (now, progress) = (self.now_ms(), self.morph_now());
        self.material.advance(now, progress);
    }

    /// The layers this frame paints: the timeline's for a material theme, `OPEN` otherwise.
    pub(super) fn material_layers(&self) -> MaterialLayers {
        if self.has_material {
            self.material.layers(self.now_ms(), self.morph_now())
        } else {
            MaterialLayers::OPEN
        }
    }

    /// True when the material has nothing left to animate (always for a plain theme).
    pub(super) fn material_settled(&self) -> bool {
        !self.has_material || self.material.is_settled(self.now_ms(), self.morph_now())
    }

    /// Notes the theme the painter is about to draw: its id for the motion line and whether it
    /// has a material.
    pub(super) fn note_theme(&mut self, paint: &ThemePaint) {
        self.theme.clone_from(&paint.drawn);
        self.has_material = paint.material.is_some();
    }
}
