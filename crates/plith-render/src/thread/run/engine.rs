//! The loop's state between frames and the per-frame work: aiming and snapping the springs,
//! taking a new mailbox generation, clock waits with demotion, and painting with GPU recovery.

use super::super::clocks::Clocks;
use super::super::{FrameTimes, Paint, RenderSinks};
use super::LoopShared;
use crate::clock::{ClockError, FrameSource};
use crate::paint::PaintFrame;
use plith_core::ambient::{LiveValues, shows_live, wants_ambient};
use plith_core::clock_choice::ClockKind;
use plith_core::draw::{DisplayList, Node, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::gpu_health::{GpuAction, GpuHealth};
use plith_core::island::IslandMotion;
use plith_core::layout::SurfaceLayout;
use plith_core::material::MaterialMotion;
use plith_core::motion::{MotionKind, MotionStats};
use plith_core::page::{PageLayers, PageTurn, motion_pages};
use plith_core::scene::{Scene, ShapeFrame, Snapshot};
use plith_core::spring::Spring;
use plith_core::theme::registry::ThemeId;
use plith_core::timeline::{FrameStamper, VsyncTimeline};
use plith_core::tuning::{Tuning, TuningSet};
use plith_core::visible::{DrawnState, visibly_differs};
use std::sync::Arc;
use std::time::SystemTime;

/// The loop's state between frames.
pub(super) struct Engine<'a, C, P, O, N> {
    pub(super) paint: P,
    pub(super) clocks: Clocks<C, O>,
    pub(super) now: N,
    /// Wall time, for the title scroll's run.
    pub(super) wall: Box<dyn Fn() -> SystemTime>,
    pub(super) wait_ms: f64,
    pub(super) hz: f64,
    pub(super) period: f64,
    pub(super) layout: SurfaceLayout,
    pub(super) sinks: &'a mut RenderSinks,
    pub(super) shared: &'a LoopShared,
    pub(super) set: TuningSet,
    pub(super) scene: Scene,
    /// The display lists and artwork drawn with `scene`.
    pub(super) page_content: Arc<PageContent>,
    /// The mailbox generation `page_content` arrived with.
    pub(super) content_gen: u64,
    pub(super) island: IslandMotion,
    pub(super) pages: PageTurn,
    pub(super) pages_settled: bool,
    pub(super) content: Spring,
    pub(super) content_settled: bool,
    pub(super) prev_raw: Option<(ClockKind, f64)>,
    pub(super) stats: Option<MotionStats>,
    pub(super) timeline: VsyncTimeline,
    pub(super) stamper: FrameStamper,
    pub(super) gpu: GpuHealth,
    pub(super) painted: u64,
    /// The shape of the last frame presented; `None` at the start and after a snap.
    pub(super) last_shape: Option<IslandShape>,
    /// The (shape, previous, pages) of the last `on_shape` call; `None` at the start and after
    /// a snap, so the first frame then always posts.
    pub(super) posted: Option<(IslandShape, IslandShape, PageLayers)>,
    pub(super) seen: u64,
    /// Per page, the list its bitmap was last rastered from (by a prewarm or a frame that
    /// showed it); `None` before that and after a GPU recovery.
    pub(super) warm: [Option<Arc<DisplayList>>; plith_core::pages::PAGE_COUNT],
    /// The pages the aimed motion shows, warmed before its first clock wait.
    pub(super) aim_pages: Vec<usize>,
    /// The content of the last presented frame.
    pub(super) presented: Option<Arc<PageContent>>,
    /// The material's open and collapse timeline; driven for every theme, painted and waited
    /// for only when `has_material`.
    pub(super) material: MaterialMotion,
    /// The height the material's morph progress reaches 1 at (`material.rs`).
    pub(super) open_height: f64,
    /// The pack drawn, named at the end of the motion line.
    pub(super) theme: ThemeId,
    /// Whether the theme drawn has a material.
    pub(super) has_material: bool,
}

/// What a new mailbox generation changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Taken {
    /// The scene is suppressed: the island jumped to it with no motion.
    Snapped,
    /// Only the page content changed: no spring was touched.
    ContentOnly,
    /// The springs were aimed at a new scene (they may already be settled).
    Aimed,
}

/// The content opacity spring, resting at 1 for an expanded scene and 0 for a collapsed one.
pub(super) fn content_spring(scene: Scene, opacity: Tuning) -> Spring {
    Spring::new(content_target(scene), opacity).with_eps(0.002, 0.01)
}

/// True when both contents carry the same artwork `Arc`s.
fn same_art(a: &PageContent, b: &PageContent) -> bool {
    a.art.len() == b.art.len() && a.art.iter().zip(&b.art).all(|(x, y)| Arc::ptr_eq(x, y))
}

/// True when `list` has a live node.
fn has_live(list: &DisplayList) -> bool {
    list.nodes.iter().any(|node| matches!(node, Node::Live(_)))
}

pub(super) fn content_target(scene: Scene) -> f64 {
    if scene.expanded { 1.0 } else { 0.0 }
}

/// Narrows an opacity in 0..=1 (clamped) for the painter.
// Allowed: the value is clamped to 0..=1, well inside f32 range and precision.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn opacity_f32(value: f64) -> f32 {
    value.clamp(0.0, 1.0) as f32
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    /// Takes a newer snapshot: stores its content, then snaps to a suppressed scene, leaves the
    /// springs alone when only the content changed, or aims them at the new scene.
    pub(super) fn take(&mut self, snapshot: Snapshot) -> Taken {
        let Snapshot {
            scene,
            content,
            generation,
        } = snapshot;
        self.seen = generation;
        if !Arc::ptr_eq(&self.page_content, &content) {
            if !same_art(&self.page_content, &content) {
                // Artwork a bitmap was missing may have arrived: let the painter look again.
                self.warm = Default::default();
            }
            self.page_content = content;
            self.content_gen = generation;
        }
        if scene.suppression.any() {
            self.snap(scene);
            Taken::Snapped
        } else if scene == self.scene {
            Taken::ContentOnly
        } else {
            self.aim(scene);
            Taken::Aimed
        }
    }

    /// True when the settled island should tick ambient frames.
    pub(super) fn wants_ambient(&self) -> bool {
        wants_ambient(&self.scene, &self.page_content, (self.wall)())
    }

    /// Back to the preferred clock with a fresh phase lock, before a motion or an ambient run.
    pub(super) fn restart_clock(&mut self) {
        self.clocks.restart();
        self.prev_raw = None;
        self.timeline.reset();
        self.stamper.reset();
    }

    /// True when the drawn state still differs from the rest state of the scene aimed at.
    pub(super) fn differs_from_rest(&self) -> bool {
        let drawn = DrawnState {
            shape: self.island.shape(),
            content: self.content.value(),
            pages: self.pages.layers(),
        };
        let rest = DrawnState::at_rest(self.scene.target(), self.scene.expanded, self.scene.page);
        visibly_differs(&drawn, &rest)
    }

    /// Reports the finished motion.
    pub(super) fn finish_motion(&mut self, drawn_by: ClockKind) {
        let Some(stats) = self.stats.take() else {
            return;
        };
        let mut report = stats.finish();
        report.clock = drawn_by.as_str();
        report.theme = Some(self.theme.as_str().to_owned());
        (self.sinks.on_motion)(report);
    }

    /// One clock wait. `None` when the sample is dropped because the clock was just demoted.
    pub(super) fn wait_clock(&mut self) -> Result<Option<f64>, String> {
        let began = (self.now)();
        let source = self.clocks.source()?;
        let kind = source.kind();
        let result = source.wait();
        self.shared.wake();
        let waited_ms = ((self.now)() - began) * 1000.0;
        self.wait_ms = waited_ms;
        if let Some(stats) = self.stats.as_mut() {
            stats.note_first_wait(waited_ms);
        }
        let demote = match &result {
            Ok(raw) => self.observe_tick(kind, *raw),
            Err(_) => true,
        };
        match result {
            // The last clock is never demoted: it keeps its samples.
            Ok(raw) if !demote || self.clocks.is_last() => Ok(Some(raw)),
            Err(error) if self.clocks.is_last() => {
                Err(format!("frame clock {} failed: {error}", kind.as_str()))
            }
            _ => {
                self.clocks.demote();
                self.prev_raw = None;
                // A new source has its own phase: lock afresh, keep the animation time running.
                self.timeline.reset();
                Ok(None)
            }
        }
    }

    /// Feeds the clock health the interval since the previous wake of the same source; true
    /// means demote. The first wake of a source has nothing to measure against.
    pub(super) fn observe_tick(&mut self, kind: ClockKind, raw: f64) -> bool {
        let previous = self.prev_raw.replace((kind, raw));
        match previous {
            Some((prev_kind, prev)) if prev_kind == kind => {
                self.clocks.health.observe(raw - prev, self.period)
            }
            _ => false,
        }
    }

    /// Aims the springs at `scene`, starting a motion or noting a retarget.
    pub(super) fn aim(&mut self, scene: Scene) {
        let target = scene.target();
        let page_changed = scene.page != self.pages.current();
        self.aim_pages = motion_pages(self.pages.current(), scene.expanded, scene.page);
        if let Some(stats) = self.stats.as_mut() {
            stats.note_retarget();
        } else {
            let from: IslandShape = self.island.shape();
            let kind = MotionKind::classify(from, target, page_changed);
            self.stats = Some(MotionStats::new(kind, self.hz, self.clocks.kind().as_str()));
        }
        self.island.retarget(target);
        self.content
            .retarget(content_target(scene), self.set.opacity);
        self.aim_material(scene);
        self.scene = scene;
        self.content_settled = self.content.is_settled();
        if page_changed {
            self.pages.turn_to(scene.page);
            self.pages_settled = false;
        }
    }

    /// Jumps to `scene` with no motion and drops the running motion unreported.
    pub(super) fn snap(&mut self, scene: Scene) {
        self.island = IslandMotion::new(scene.target(), self.set);
        self.scene = scene;
        self.pages = PageTurn::new(scene.page, self.set);
        self.pages_settled = true;
        self.content = content_spring(scene, self.set.opacity);
        self.content_settled = true;
        self.snap_material(scene);
        self.stats = None;
        self.last_shape = None;
        self.posted = None;
    }

    pub(super) fn is_settled(&self) -> bool {
        self.island.is_settled()
            && self.pages_settled
            && self.content_settled
            && self.material_settled()
    }

    /// Paints the current state, with live values sampled once when a visible page layer has a
    /// live node (the default values otherwise), and the wall time when the visible pill has
    /// one (the title scroll). After a failed paint and a successful `recover`, the same
    /// frame is painted once more (no extra clock wait), so a recovered frame is not lost.
    pub(super) fn paint_frame(&mut self) -> Result<Option<FrameTimes>, String> {
        let shape = self.island.shape();
        let pages = self.pages.layers();
        let content_opacity = opacity_f32(self.content.value());
        let mut live = if shows_live(&self.page_content, &pages, f64::from(content_opacity)) {
            (self.sinks.sample)()
        } else {
            LiveValues::default()
        };
        if content_opacity < 1.0 && has_live(&self.page_content.pill) {
            live.now = (self.wall)();
        }
        let frame = PaintFrame {
            shape,
            origin: self.layout.island_origin(shape),
            pages,
            content_opacity,
            content: Arc::clone(&self.page_content),
            content_gen: self.content_gen,
            live,
            material: self.material_layers(),
        };
        let (times, recovered) = self.paint_once(&frame)?;
        if times.is_none() && recovered {
            return Ok(self.paint_once(&frame)?.0);
        }
        Ok(times)
    }

    /// One paint: publishes the shape and page layers on success (except a settled frame
    /// already published, so an ambient run does not post the same frame every time), feeds
    /// `GpuHealth` and runs its action.
    /// Returns the frame times (on success) and whether `recover` ran.
    pub(super) fn paint_once(
        &mut self,
        frame: &PaintFrame,
    ) -> Result<(Option<FrameTimes>, bool), String> {
        let result = self.paint.paint(frame);
        let action = self.gpu.record(result.is_ok());
        let times = match result {
            Ok(times) => {
                self.painted += 1;
                self.note_presented(frame);
                let previous = self.last_shape.replace(frame.shape).unwrap_or(frame.shape);
                let drawn = (frame.shape, previous, frame.pages);
                if previous != frame.shape || self.posted != Some(drawn) {
                    self.posted = Some(drawn);
                    (self.sinks.on_shape)(ShapeFrame {
                        shape: frame.shape,
                        previous,
                        generation: self.painted,
                        pages: frame.pages,
                    });
                }
                Some(times)
            }
            Err(_) => None,
        };
        let warp = match action {
            GpuAction::None => return Ok((times, false)),
            GpuAction::Recreate => false,
            GpuAction::FallBackToWarp => true,
        };
        self.warm = Default::default();
        self.presented = None;
        self.paint
            .recover(warp)
            .map_err(|error| format!("GPU recovery failed (warp {warp}): {error}"))?;
        for line in self.paint.take_notes() {
            (self.sinks.on_note)(line);
        }
        Ok((times, true))
    }
}
