//! The generic render loop: parks on the scene mailbox at rest; while moving it waits on a
//! frame clock, phase-locks the time, steps the springs in whole periods, paints, publishes the
//! drawn shape and writes one motion report per motion. New content on a still island paints
//! one frame; an open page with live nodes ticks ambient frames (`ambient.rs`).

mod ambient;
mod engine;
mod prewarm;
mod relayout;

pub use relayout::Relayout;

use super::clocks::Clocks;
use super::{Fatal, Paint, RenderSinks};
use crate::clock::{ClockError, FrameSource};
use ambient::AmbientEnd;
use engine::{Engine, Taken, content_spring};
use plith_core::clock_choice::ClockKind;
use plith_core::gpu_health::GpuHealth;
use plith_core::island::IslandMotion;
use plith_core::layout::SurfaceLayout;
use plith_core::material::art::ThemePaint;
use plith_core::motion::FramePhases;
use plith_core::page::PageTurn;
use plith_core::panic_text::payload_text;
use plith_core::scene::{Scene, SceneMailbox, Snapshot};
use plith_core::timeline::{FrameStamper, VsyncTimeline};
use plith_core::tuning::TuningSet;
use prewarm::Reason;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

/// State shared between the render loop and its owner: the wake counter, the stop flag, a
/// pending relayout and the monitor and rate the next clock opens for.
#[derive(Debug, Default)]
pub struct LoopShared {
    wakes: AtomicU64,
    stop: AtomicBool,
    relayout: Mutex<Option<Relayout>>,
    clock_target: Mutex<(isize, f64)>,
    theme: Mutex<Option<ThemePaint>>,
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
    /// Wall time, for the title scroll's run (`SystemTime::now` outside tests).
    pub wall: Box<dyn Fn() -> SystemTime>,
    /// Clock kinds to try, best first; the last one is never demoted.
    pub order: Vec<ClockKind>,
    /// Nominal display refresh rate.
    pub hz: f64,
    /// The surface layout, for the island origin.
    pub layout: SurfaceLayout,
    /// The spring tuning preset.
    pub tune: TuningSet,
}

/// Runs [`run_loop`] and turns a panic anywhere in it (the painter, a clock, a sink) into
/// `on_fatal("render thread panicked: ...")`. `cleanup` runs after the loop ends, whichever way,
/// and before a panic is reported.
pub fn run_guarded<C, P, O, N>(
    parts: LoopParts<P, O, N>,
    mailbox: &SceneMailbox,
    shared: &LoopShared,
    sinks: &mut RenderSinks,
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
    sinks: &mut RenderSinks,
) where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    let Snapshot {
        scene,
        content: page_content,
        generation: seen,
    } = mailbox.latest();
    let mut engine = Engine {
        paint: parts.paint,
        clocks: Clocks::new(parts.open_clock, parts.order),
        now: parts.now,
        wall: parts.wall,
        wait_ms: 0.0,
        hz: parts.hz,
        period: 1.0 / parts.hz,
        layout: parts.layout,
        sinks,
        shared,
        set: parts.tune,
        scene,
        page_content,
        content_gen: seen,
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
        posted: None,
        seen,
        warm: Default::default(),
        aim_pages: Vec::new(),
        presented: None,
    };
    if engine.clocks.order.is_empty() {
        (engine.sinks.on_fatal)(Fatal::Error(String::from("no frame clock kinds to try")));
        return;
    }
    if let Err(message) = engine.run(mailbox, scene) {
        (engine.sinks.on_fatal)(Fatal::Error(message));
    }
}

impl<C, P, O, N> Engine<'_, C, P, O, N>
where
    C: FrameSource,
    P: Paint,
    O: FnMut(ClockKind) -> Result<C, ClockError>,
    N: Fn() -> f64,
{
    /// Paints the start frame, then parks, animates and ticks the ambient frames until stopped.
    fn run(&mut self, mailbox: &SceneMailbox, start: Scene) -> Result<(), String> {
        if !start.suppression.any() {
            self.paint_frame()?;
        }
        loop {
            if self.wants_ambient() {
                match self.ambient(mailbox)? {
                    AmbientEnd::Stopped => return Ok(()),
                    AmbientEnd::Parked => {}
                    AmbientEnd::Moving => {
                        if self.warmed_motion(mailbox)? {
                            return Ok(());
                        }
                        continue;
                    }
                }
            }
            // A request whose nudge a motion or an ambient run already took (it landed between
            // their apply and their mailbox read) would wait here for the next publish.
            self.apply_relayout(true)?;
            let next = mailbox.wait_newer(self.seen);
            self.shared.wake();
            let Some(snapshot) = next else {
                return Ok(());
            };
            let taken = self.take(snapshot);
            // Only a still island needs its own repaint; every other branch paints or is
            // suppressed.
            self.apply_relayout(taken == Taken::ContentOnly)?;
            match taken {
                Taken::Snapped => continue,
                // New content on a still island: warm what changed, present only when a shown
                // list changed; no clock wait, no motion.
                Taken::ContentOnly => {
                    self.content_at_rest()?;
                    continue;
                }
                Taken::Aimed if self.is_settled() => {
                    // Nothing to animate (an unsuppress): one frame, no clock wait.
                    self.stats = None;
                    self.aim_pages.clear();
                    self.paint_frame()?;
                    continue;
                }
                Taken::Aimed => {}
            }
            if self.warmed_motion(mailbox)? {
                return Ok(());
            }
        }
    }

    /// Warms the pages the aimed motion shows (before its first clock wait, outside its stats),
    /// animates it, then warms the other pages at rest; true when a stop was requested.
    fn warmed_motion(&mut self, mailbox: &SceneMailbox) -> Result<bool, String> {
        let aimed = std::mem::take(&mut self.aim_pages);
        self.prewarm(&aimed, Reason::Aim);
        if self.motion(mailbox)? {
            return Ok(true);
        }
        self.prewarm_open(Reason::Rest);
        Ok(false)
    }

    /// Starts the clock afresh and animates the aimed motion; true when a stop was requested.
    fn motion(&mut self, mailbox: &SceneMailbox) -> Result<bool, String> {
        self.restart_clock();
        self.animate(mailbox)?;
        Ok(self.shared.stopping())
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
            // `needs_rate_reread` is ignored: a display change arrives as a relayout with the
            // new rate (`relayout.rs`), applied here before the frame it shapes.
            let (anim_t, dt) = self.stamper.stamp(tick);
            self.apply_relayout(false)?;
            let snapshot = mailbox.latest();
            // New content alone keeps the motion as it is; it shows from this frame on.
            if snapshot.generation != self.seen && self.take(snapshot) == Taken::Snapped {
                return Ok(());
            }
            // A retarget mid-motion warms nothing here: the frames are being measured.
            self.aim_pages.clear();
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
}
