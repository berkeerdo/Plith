//! The generic render loop: parks on the scene mailbox at rest; while moving it waits on a
//! frame clock, phase-locks the time, steps the springs in whole periods, paints, publishes the
//! drawn shape and writes one motion report per motion.

use super::clocks::Clocks;
use super::{Fatal, FrameTimes, Paint, RenderSinks};
use crate::clock::{ClockError, FrameSource};
use crate::paint::PaintFrame;
use plith_core::clock_choice::ClockKind;
use plith_core::geometry::IslandShape;
use plith_core::gpu_health::{GpuAction, GpuHealth};
use plith_core::island::IslandMotion;
use plith_core::layout::SurfaceLayout;
use plith_core::motion::{FramePhases, MotionKind, MotionStats};
use plith_core::page::PageTurn;
use plith_core::panic_text::payload_text;
use plith_core::scene::{Scene, SceneMailbox, ShapeFrame, Snapshot};
use plith_core::spring::Spring;
use plith_core::timeline::{FrameStamper, VsyncTimeline};
use plith_core::tuning::{Tuning, TuningSet};
use plith_core::visible::{DrawnState, visibly_differs};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// State shared between the render loop and its owner: the wake counter and the stop flag.
#[derive(Debug, Default)]
pub struct LoopShared {
    wakes: AtomicU64,
    stop: AtomicBool,
}

impl LoopShared {
    /// A zero wake count and no stop request.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns from any wait (mailbox or clock) so far.
    #[must_use]
    pub fn wakes(&self) -> u64 {
        self.wakes.load(Ordering::Relaxed)
    }

    /// Asks a moving loop to stop at its next frame. Pair it with `SceneMailbox::stop`, which
    /// releases a parked loop.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn wake(&self) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
    }
}

/// Everything the loop owns: the painter, how to open and time clocks, and the display.
pub struct LoopParts<P, O, N> {
    /// Paints and presents one frame.
    pub paint: P,
    /// Opens a clock of the given kind.
    pub open_clock: O,
    /// Raw seconds since the origin the clocks also use.
    pub now: N,
    /// Clock kinds to try, best first; the last one is never demoted.
    pub order: Vec<ClockKind>,
    /// Nominal display refresh rate.
    pub hz: f64,
    /// The surface layout, for the island origin.
    pub layout: SurfaceLayout,
    /// The spring tuning preset.
    pub tune: TuningSet,
}

/// The loop's state between frames.
struct Engine<'a, C, P, O, N> {
    paint: P,
    clocks: Clocks<C, O>,
    now: N,
    wait_ms: f64,
    hz: f64,
    period: f64,
    layout: SurfaceLayout,
    sinks: &'a RenderSinks,
    shared: &'a LoopShared,
    set: TuningSet,
    scene: Scene,
    island: IslandMotion,
    pages: PageTurn,
    pages_settled: bool,
    content: Spring,
    content_settled: bool,
    prev_raw: Option<(ClockKind, f64)>,
    stats: Option<MotionStats>,
    timeline: VsyncTimeline,
    stamper: FrameStamper,
    gpu: GpuHealth,
    painted: u64,
    /// The shape of the last frame presented; `None` at the start and after a snap.
    last_shape: Option<IslandShape>,
    seen: u64,
}

/// The content opacity spring, resting at 1 for an expanded scene and 0 for a collapsed one.
fn content_spring(scene: Scene, opacity: Tuning) -> Spring {
    Spring::new(content_target(scene), opacity).with_eps(0.002, 0.01)
}

fn content_target(scene: Scene) -> f64 {
    if scene.expanded { 1.0 } else { 0.0 }
}

/// Narrows an opacity in 0..=1 (clamped) for the painter.
// Allowed: the value is clamped to 0..=1, well inside f32 range and precision.
#[allow(clippy::cast_possible_truncation)]
fn opacity_f32(value: f64) -> f32 {
    value.clamp(0.0, 1.0) as f32
}

/// Runs [`run_loop`] and turns a panic anywhere in it (the painter, a clock, a sink) into
/// `on_fatal("render thread panicked: ...")`. `cleanup` runs after the loop ends, whichever way,
/// and before a panic is reported.
pub fn run_guarded<C, P, O, N>(
    parts: LoopParts<P, O, N>,
    mailbox: &SceneMailbox,
    shared: &LoopShared,
    sinks: &RenderSinks,
    cleanup: impl FnOnce(),
) where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        run_loop(parts, mailbox, shared, sinks);
    }));
    cleanup();
    if let Err(payload) = outcome {
        let text = payload_text(&*payload);
        (sinks.on_fatal)(Fatal::Panic(format!("render thread panicked: {text}")));
    }
}

/// Runs until the mailbox stops or a fatal error (reported through `sinks.on_fatal`).
pub fn run_loop<C, P, O, N>(
    parts: LoopParts<P, O, N>,
    mailbox: &SceneMailbox,
    shared: &LoopShared,
    sinks: &RenderSinks,
) where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    let Snapshot {
        scene,
        generation: seen,
        ..
    } = mailbox.latest();
    let mut engine = Engine {
        paint: parts.paint,
        clocks: Clocks::new(parts.open_clock, parts.order),
        now: parts.now,
        wait_ms: 0.0,
        hz: parts.hz,
        period: 1.0 / parts.hz,
        layout: parts.layout,
        sinks,
        shared,
        set: parts.tune,
        scene,
        island: IslandMotion::new(scene.target(), parts.tune),
        pages: PageTurn::new(scene.page, parts.tune),
        pages_settled: true,
        content: content_spring(scene, parts.tune.opacity),
        content_settled: true,
        prev_raw: None,
        stats: None,
        timeline: VsyncTimeline::new(parts.hz),
        stamper: FrameStamper::default(),
        gpu: GpuHealth::default(),
        painted: 0,
        last_shape: None,
        seen,
    };
    if engine.clocks.order.is_empty() {
        (sinks.on_fatal)(Fatal::Error(String::from("no frame clock kinds to try")));
        return;
    }
    if let Err(message) = engine.run(mailbox, scene) {
        (sinks.on_fatal)(Fatal::Error(message));
    }
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    /// Paints the start frame, then parks and animates until stopped.
    fn run(&mut self, mailbox: &SceneMailbox, start: Scene) -> Result<(), String> {
        if !start.suppression.any() {
            self.paint_frame()?;
        }
        loop {
            let next = mailbox.wait_newer(self.seen);
            self.shared.wake();
            let Some(Snapshot {
                scene, generation, ..
            }) = next
            else {
                return Ok(());
            };
            self.seen = generation;
            if scene.suppression.any() {
                self.snap(scene);
                continue;
            }
            self.aim(scene);
            if self.is_settled() {
                // Nothing to animate (a duplicate or an unsuppress): one frame, no clock wait.
                self.stats = None;
                self.paint_frame()?;
                continue;
            }
            self.clocks.restart();
            self.prev_raw = None;
            self.timeline.reset();
            self.stamper.reset();
            self.animate(mailbox)?;
            if self.shared.stopping() {
                return Ok(());
            }
        }
    }

    /// Clock-paced frames until the motion settles, is suppressed or a stop is requested.
    fn animate(&mut self, mailbox: &SceneMailbox) -> Result<(), String> {
        loop {
            if self.shared.stopping() {
                self.stats = None;
                return Ok(());
            }
            let Some(raw) = self.wait_clock()? else {
                continue;
            };
            let Some(tick) = self.timeline.lock(raw) else {
                continue;
            };
            // `needs_rate_reread` is ignored in R1: a display change restarts the thread (Task 9).
            let (anim_t, dt) = self.stamper.stamp(tick);
            let Snapshot {
                scene, generation, ..
            } = mailbox.latest();
            if generation != self.seen {
                self.seen = generation;
                if scene.suppression.any() {
                    self.snap(scene);
                    return Ok(());
                }
                self.aim(scene);
            }
            let island_settled = self.island.step(dt);
            self.pages_settled = self.pages.step(dt);
            self.content_settled = self.content.step(dt);
            let drawn_by = self.clocks.kind();
            let visible = self.differs_from_rest();
            if let Some(times) = self.paint_frame()?
                && let Some(stats) = self.stats.as_mut()
            {
                let phases = FramePhases {
                    wait_ms: self.wait_ms,
                    late_ms: tick.late * 1000.0,
                    draw_ms: times.draw_ms,
                    present_ms: times.present_ms,
                };
                stats.record_frame(anim_t, phases);
                if visible {
                    stats.note_visible(anim_t);
                }
            }
            // The wait belongs to the frame it paced: a repaint without a wait reports 0.
            self.wait_ms = 0.0;
            if island_settled && self.pages_settled && self.content_settled {
                self.finish_motion(drawn_by);
                return Ok(());
            }
        }
    }

    /// True when the drawn state still differs from the rest state of the scene aimed at.
    fn differs_from_rest(&self) -> bool {
        let drawn = DrawnState {
            shape: self.island.shape(),
            content: self.content.value(),
            pages: self.pages.layers(),
        };
        let rest = DrawnState::at_rest(self.scene.target(), self.scene.expanded, self.scene.page);
        visibly_differs(&drawn, &rest)
    }

    /// Reports the finished motion.
    fn finish_motion(&mut self, drawn_by: ClockKind) {
        let Some(stats) = self.stats.take() else {
            return;
        };
        let mut report = stats.finish();
        report.clock = drawn_by.as_str();
        (self.sinks.on_motion)(report);
    }

    /// One clock wait. `None` when the sample is dropped because the clock was just demoted.
    fn wait_clock(&mut self) -> Result<Option<f64>, String> {
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
    fn observe_tick(&mut self, kind: ClockKind, raw: f64) -> bool {
        let previous = self.prev_raw.replace((kind, raw));
        match previous {
            Some((prev_kind, prev)) if prev_kind == kind => {
                self.clocks.health.observe(raw - prev, self.period)
            }
            _ => false,
        }
    }

    /// Aims the springs at `scene`, starting a motion or noting a retarget.
    fn aim(&mut self, scene: Scene) {
        let target = scene.target();
        let page_changed = scene.page != self.pages.current();
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
        self.scene = scene;
        self.content_settled = self.content.is_settled();
        if page_changed {
            self.pages.turn_to(scene.page);
            self.pages_settled = false;
        }
    }

    /// Jumps to `scene` with no motion and drops the running motion unreported.
    fn snap(&mut self, scene: Scene) {
        self.island = IslandMotion::new(scene.target(), self.set);
        self.scene = scene;
        self.pages = PageTurn::new(scene.page, self.set);
        self.pages_settled = true;
        self.content = content_spring(scene, self.set.opacity);
        self.content_settled = true;
        self.stats = None;
        self.last_shape = None;
    }

    fn is_settled(&self) -> bool {
        self.island.is_settled() && self.pages_settled && self.content_settled
    }

    /// Paints the current state. After a failed paint and a successful `recover`, the same
    /// frame is painted once more (no extra clock wait), so a recovered frame is not lost.
    fn paint_frame(&mut self) -> Result<Option<FrameTimes>, String> {
        let shape = self.island.shape();
        let frame = PaintFrame {
            shape,
            origin: self.layout.island_origin(shape),
            pages: self.pages.layers(),
            content_opacity: opacity_f32(self.content.value()),
        };
        let (times, recovered) = self.paint_once(&frame)?;
        if times.is_none() && recovered {
            return Ok(self.paint_once(&frame)?.0);
        }
        Ok(times)
    }

    /// One paint: publishes the shape on success, feeds `GpuHealth` and runs its action.
    /// Returns the frame times (on success) and whether `recover` ran.
    fn paint_once(&mut self, frame: &PaintFrame) -> Result<(Option<FrameTimes>, bool), String> {
        let result = self.paint.paint(frame);
        let action = self.gpu.record(result.is_ok());
        let times = match result {
            Ok(times) => {
                self.painted += 1;
                let previous = self.last_shape.replace(frame.shape);
                (self.sinks.on_shape)(ShapeFrame {
                    shape: frame.shape,
                    previous: previous.unwrap_or(frame.shape),
                    generation: self.painted,
                });
                Some(times)
            }
            Err(_) => None,
        };
        let warp = match action {
            GpuAction::None => return Ok((times, false)),
            GpuAction::Recreate => false,
            GpuAction::FallBackToWarp => true,
        };
        self.paint
            .recover(warp)
            .map_err(|error| format!("GPU recovery failed (warp {warp}): {error}"))?;
        for line in self.paint.take_notes() {
            (self.sinks.on_note)(line);
        }
        Ok((times, true))
    }
}
