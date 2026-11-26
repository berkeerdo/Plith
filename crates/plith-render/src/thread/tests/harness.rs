//! Fakes and the run harness for the render loop tests: a fake clock on shared fake time, a
//! recording painter and a loop runner with bounded waits.

use super::super::{
    Fatal, FrameTimes, LoopParts, LoopShared, Paint, Relayout, RenderSinks, run_guarded,
};
use crate::clock::{ClockError, FrameSource};
use crate::paint::PaintFrame;
use plith_core::ambient::{AmbientReport, LiveValues};
use plith_core::clock_choice::ClockKind;
use plith_core::draw::PageContent;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_core::material::art::ThemePaint;
use plith_core::motion::MotionReport;
use plith_core::scene::{Scene, SceneMailbox, ShapeFrame};
use plith_core::theme::Palette;
use plith_core::tuning::R1;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, UNIX_EPOCH};
use windows::Win32::Foundation::E_FAIL;

pub(super) const PERIOD: f64 = 1.0 / 240.0;
pub(super) const BOUND: Duration = Duration::from_secs(2);
pub(super) const QUIET: Duration = Duration::from_millis(50);
pub(super) const KINDS: [ClockKind; 3] = [ClockKind::Dcomp, ClockKind::Dxgi, ClockKind::Timer];
/// Every prewarm call: the pages asked for and the clock call count at the time.
pub(super) type Prewarms = Arc<Mutex<Vec<(Vec<usize>, u64)>>>;
/// What the fake painter reports for every prewarm call, in milliseconds.
pub(super) const PREWARM_MS: f64 = 0.75;

pub(super) fn index(kind: ClockKind) -> usize {
    match kind {
        ClockKind::Dcomp => 0,
        ClockKind::Dxgi => 1,
        ClockKind::Timer => 2,
    }
}

/// Shared fake time, per-kind call counters and scenes published at given call numbers.
pub(super) struct Harness {
    pub(super) time: Mutex<f64>,
    pub(super) total: AtomicU64,
    pub(super) calls: [AtomicU64; 3],
    pub(super) script: Vec<(u64, Scene)>,
    pub(super) content_script: Vec<(u64, Arc<PageContent>)>,
    pub(super) mailbox: Arc<SceneMailbox>,
    pub(super) delay: Option<Duration>,
    pub(super) fail_once: Option<ClockKind>,
    pub(super) failed: AtomicBool,
    /// How far the loop's own clock runs ahead of fake time, to model work before each wait.
    pub(super) lead: f64,
}

impl Harness {
    pub(super) fn now(&self) -> f64 {
        *self.time.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn advance(&self, secs: f64) {
        *self.time.lock().unwrap_or_else(PoisonError::into_inner) += secs;
    }

    pub(super) fn total(&self) -> u64 {
        self.total.load(Ordering::SeqCst)
    }

    pub(super) fn calls(&self, kind: ClockKind) -> u64 {
        self.calls[index(kind)].load(Ordering::SeqCst)
    }
}

/// Returns `k / 240` on the k-th call, or the unchanged time when `instant`.
pub(super) struct FakeClock {
    pub(super) kind: ClockKind,
    pub(super) instant: bool,
    pub(super) harness: Arc<Harness>,
}

impl FrameSource for FakeClock {
    fn wait(&mut self) -> Result<f64, ClockError> {
        if let Some(delay) = self.harness.delay {
            thread::sleep(delay);
        }
        let fails = self.harness.fail_once == Some(self.kind)
            && !self.harness.failed.swap(true, Ordering::SeqCst);
        if !self.instant && !fails {
            self.harness.advance(PERIOD);
        }
        let call = self.harness.total.fetch_add(1, Ordering::SeqCst) + 1;
        self.harness.calls[index(self.kind)].fetch_add(1, Ordering::SeqCst);
        if fails {
            return Err(ClockError::Timeout);
        }
        for (at, scene) in &self.harness.script {
            if *at == call {
                let _ = self.harness.mailbox.publish(*scene);
            }
        }
        for (at, content) in &self.harness.content_script {
            if *at == call {
                let _ = self.harness.mailbox.publish_content(Arc::clone(content));
            }
        }
        Ok(self.harness.now())
    }

    fn kind(&self) -> ClockKind {
        self.kind
    }
}

/// Records painted frames and the clock call count at each; fails the first `fail` paints; records every `recover` call.
pub(super) struct FakePaint {
    pub(super) frames: Arc<Mutex<Vec<PaintFrame>>>,
    pub(super) calls_at: Arc<Mutex<Vec<u64>>>,
    pub(super) harness: Arc<Harness>,
    pub(super) fail: Arc<AtomicU32>,
    pub(super) recovers: Arc<Mutex<Vec<bool>>>,
    pub(super) recover_fails: bool,
    /// Panics when asked to paint this frame number (1 is the start frame).
    pub(super) panic_at: Option<usize>,
    /// Every prewarm call: the pages asked for and the clock call count at the time.
    pub(super) prewarms: Prewarms,
    /// Every relayout call, in order.
    pub(super) relayouts: Arc<Mutex<Vec<SurfaceLayout>>>,
    pub(super) relayout_fails: bool,
    /// After painting this frame number, stores the relayout in `shared` with no nudge: the
    /// state a request leaves when its generation was consumed by a take that did not apply it.
    pub(super) relayout_after: Option<(usize, Relayout)>,
    pub(super) shared: Arc<LoopShared>,
    /// Every palette the loop switched to, in order.
    pub(super) palettes: Arc<Mutex<Vec<Palette>>>,
}

impl Paint for FakePaint {
    fn set_theme(&mut self, paint: ThemePaint) {
        lock(&self.palettes).push(paint.open);
    }

    fn relayout(&mut self, layout: SurfaceLayout) -> windows::core::Result<()> {
        lock(&self.relayouts).push(layout);
        if self.relayout_fails {
            Err(E_FAIL.into())
        } else {
            Ok(())
        }
    }

    fn paint(&mut self, frame: &PaintFrame) -> windows::core::Result<FrameTimes> {
        assert!(
            self.panic_at != Some(lock(&self.frames).len() + 1),
            "fake paint blew up"
        );
        let left = self.fail.load(Ordering::SeqCst);
        if left > 0 {
            self.fail.store(left - 1, Ordering::SeqCst);
            return Err(E_FAIL.into());
        }
        lock(&self.frames).push(frame.clone());
        lock(&self.calls_at).push(self.harness.total());
        let painted = lock(&self.frames).len();
        if let Some((at, relayout)) = &self.relayout_after
            && *at == painted
        {
            self.shared.request_relayout(relayout.clone());
        }
        Ok(FrameTimes {
            draw_ms: 1.0,
            present_ms: 0.5,
        })
    }

    fn prewarm(&mut self, _content: &PageContent, pages: &[usize]) -> f64 {
        lock(&self.prewarms).push((pages.to_vec(), self.harness.total()));
        PREWARM_MS
    }

    fn recover(&mut self, warp: bool) -> windows::core::Result<()> {
        lock(&self.recovers).push(warp);
        if self.recover_fails {
            Err(E_FAIL.into())
        } else {
            Ok(())
        }
    }
}

pub(super) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
pub(super) struct Setup {
    pub(super) instant: Vec<ClockKind>,
    pub(super) fail: u32,
    pub(super) recover_fails: bool,
    pub(super) script: Vec<(u64, Scene)>,
    pub(super) delay: Option<Duration>,
    pub(super) fail_once: Option<ClockKind>,
    pub(super) lead: f64,
    pub(super) panic_at: Option<usize>,
    /// The page content the mailbox starts with (empty when `None`).
    pub(super) content: Option<Arc<PageContent>>,
    pub(super) content_script: Vec<(u64, Arc<PageContent>)>,
    pub(super) relayout_fails: bool,
    pub(super) relayout_after: Option<(usize, Relayout)>,
}

pub(super) struct Run {
    /// Every relayout the painter was asked for.
    pub(super) relayouts: Arc<Mutex<Vec<SurfaceLayout>>>,
    /// How many clocks were opened.
    pub(super) opens: Arc<AtomicU64>,
    /// Every palette the painter was switched to.
    pub(super) palettes: Arc<Mutex<Vec<Palette>>>,
    pub(super) mailbox: Arc<SceneMailbox>,
    pub(super) shared: Arc<LoopShared>,
    pub(super) harness: Arc<Harness>,
    pub(super) frames: Arc<Mutex<Vec<PaintFrame>>>,
    pub(super) calls_at: Arc<Mutex<Vec<u64>>>,
    pub(super) shapes: Arc<Mutex<Vec<ShapeFrame>>>,
    pub(super) recovers: Arc<Mutex<Vec<bool>>>,
    pub(super) reports: Receiver<MotionReport>,
    pub(super) ambient: Receiver<AmbientReport>,
    pub(super) samples: Arc<AtomicU64>,
    pub(super) fatal: Receiver<Fatal>,
    pub(super) notes: Receiver<String>,
    pub(super) prewarms: Prewarms,
    pub(super) done: Receiver<()>,
    pub(super) join: JoinHandle<()>,
}

pub(super) fn layout() -> SurfaceLayout {
    let monitor = RectPx {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    SurfaceLayout::new(monitor, 0, 1.0)
}

/// The receivers of the sinks built by [`sinks`].
pub(super) struct SinkEnds {
    pub(super) reports: Receiver<MotionReport>,
    pub(super) ambient: Receiver<AmbientReport>,
    pub(super) fatal: Receiver<Fatal>,
    pub(super) notes: Receiver<String>,
}

/// Sinks into channels; `sample` counts its calls in `samples` and returns the count as strip 0.
pub(super) fn sinks(
    shapes: Arc<Mutex<Vec<ShapeFrame>>>,
    samples: Arc<AtomicU64>,
) -> (RenderSinks, SinkEnds) {
    let (report_tx, reports) = mpsc::channel();
    let (ambient_tx, ambient) = mpsc::channel();
    let (fatal_tx, fatal) = mpsc::channel();
    let (note_tx, notes) = mpsc::channel();
    let sinks = RenderSinks {
        on_shape: Box::new(move |frame| lock(&shapes).push(frame)),
        on_motion: Box::new(move |report| {
            let _ = report_tx.send(report);
        }),
        on_fatal: Box::new(move |message| {
            let _ = fatal_tx.send(message);
        }),
        on_note: Box::new(move |line| {
            let _ = note_tx.send(line);
        }),
        sample: Box::new(move || {
            let n = samples.fetch_add(1, Ordering::SeqCst) + 1;
            #[allow(clippy::cast_precision_loss)]
            let level = n as f32;
            LiveValues {
                strips: vec![[level, level]],
                ..LiveValues::default()
            }
        }),
        on_ambient: Box::new(move |report| {
            let _ = ambient_tx.send(report);
        }),
    };
    let ends = SinkEnds {
        reports,
        ambient,
        fatal,
        notes,
    };
    (sinks, ends)
}

/// The shared fake time and scripts of `setup`, publishing into `mailbox`.
fn harness(setup: &mut Setup, mailbox: &Arc<SceneMailbox>) -> Arc<Harness> {
    Arc::new(Harness {
        time: Mutex::new(0.0),
        total: AtomicU64::new(0),
        calls: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
        script: std::mem::take(&mut setup.script),
        content_script: std::mem::take(&mut setup.content_script),
        mailbox: Arc::clone(mailbox),
        delay: setup.delay,
        fail_once: setup.fail_once,
        failed: AtomicBool::new(false),
        lead: setup.lead,
    })
}

/// What the fake painter records, shared with the run.
#[derive(Default)]
struct Probes {
    frames: Arc<Mutex<Vec<PaintFrame>>>,
    calls_at: Arc<Mutex<Vec<u64>>>,
    recovers: Arc<Mutex<Vec<bool>>>,
    prewarms: Prewarms,
    relayouts: Arc<Mutex<Vec<SurfaceLayout>>>,
    palettes: Arc<Mutex<Vec<Palette>>>,
}

pub(super) fn start(mut setup: Setup) -> Run {
    let content = setup.content.clone().unwrap_or_default();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), content));
    let shared = Arc::new(LoopShared::new());
    let harness = harness(&mut setup, &mailbox);
    let probes = Probes::default();
    let shapes = Arc::new(Mutex::new(Vec::new()));
    let samples = Arc::new(AtomicU64::new(0));
    let opens = Arc::new(AtomicU64::new(0));
    let (mut sinks, ends) = sinks(Arc::clone(&shapes), Arc::clone(&samples));
    let paint = FakePaint {
        frames: Arc::clone(&probes.frames),
        calls_at: Arc::clone(&probes.calls_at),
        harness: Arc::clone(&harness),
        fail: Arc::new(AtomicU32::new(setup.fail)),
        recovers: Arc::clone(&probes.recovers),
        recover_fails: setup.recover_fails,
        panic_at: setup.panic_at,
        prewarms: Arc::clone(&probes.prewarms),
        relayouts: Arc::clone(&probes.relayouts),
        relayout_fails: setup.relayout_fails,
        relayout_after: setup.relayout_after.take(),
        shared: Arc::clone(&shared),
        palettes: Arc::clone(&probes.palettes),
    };
    let (done_tx, done) = mpsc::channel();
    let join = {
        let (mailbox, shared, harness) = (
            Arc::clone(&mailbox),
            Arc::clone(&shared),
            Arc::clone(&harness),
        );
        let instant = setup.instant;
        let opened = Arc::clone(&opens);
        thread::spawn(move || {
            let clock_harness = Arc::clone(&harness);
            let wall_harness = Arc::clone(&harness);
            let parts = LoopParts {
                paint,
                open_clock: move |kind| {
                    opened.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, ClockError>(FakeClock {
                        kind,
                        instant: instant.contains(&kind),
                        harness: Arc::clone(&clock_harness),
                    })
                },
                now: move || harness.now() + harness.lead,
                // Fake wall time: the epoch plus the fake time, so scroll runs are exact.
                wall: Box::new(move || UNIX_EPOCH + Duration::from_secs_f64(wall_harness.now())),
                order: KINDS.to_vec(),
                hz: 240.0,
                layout: layout(),
                tune: R1,
            };
            run_guarded(parts, &mailbox, &shared, &mut sinks, || {});
            let _ = done_tx.send(());
        })
    };
    Run {
        relayouts: probes.relayouts,
        opens,
        palettes: probes.palettes,
        mailbox,
        shared,
        harness,
        frames: probes.frames,
        calls_at: probes.calls_at,
        shapes,
        recovers: probes.recovers,
        reports: ends.reports,
        ambient: ends.ambient,
        samples,
        fatal: ends.fatal,
        notes: ends.notes,
        prewarms: probes.prewarms,
        done,
        join,
    }
}

impl Run {
    pub(super) fn frames(&self) -> Vec<PaintFrame> {
        lock(&self.frames).clone()
    }

    /// Every prewarm call so far: the pages and the clock call count at the time.
    pub(super) fn prewarms(&self) -> Vec<(Vec<usize>, u64)> {
        lock(&self.prewarms).clone()
    }

    /// The `prewarm:` log lines so far.
    pub(super) fn prewarm_notes(&self) -> Vec<String> {
        self.notes
            .try_iter()
            .filter(|line| line.starts_with("prewarm:"))
            .collect()
    }

    pub(super) fn calls_at(&self) -> Vec<u64> {
        lock(&self.calls_at).clone()
    }

    pub(super) fn ambient_report(&self) -> AmbientReport {
        self.ambient
            .recv_timeout(BOUND)
            .expect("an ambient report within the bound")
    }

    pub(super) fn report(&self) -> MotionReport {
        self.reports
            .recv_timeout(BOUND)
            .expect("a motion report within the bound")
    }

    pub(super) fn wait_frames(&self, n: usize) {
        let deadline = Instant::now() + BOUND;
        while lock(&self.frames).len() < n {
            assert!(Instant::now() < deadline, "expected {n} painted frames");
            thread::sleep(Duration::from_millis(1));
        }
    }

    pub(super) fn wait_calls(&self, n: u64) {
        let deadline = Instant::now() + BOUND;
        while self.harness.total() < n {
            assert!(Instant::now() < deadline, "expected {n} clock waits");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Asserts that no clock wait happens for a while: the thread is parked.
    pub(super) fn assert_parked(&self) {
        let before = self.harness.total();
        thread::sleep(QUIET);
        assert_eq!(self.harness.total(), before, "clock waits while parked");
    }

    pub(super) fn wakes(&self) -> u64 {
        self.shared.wakes()
    }

    /// Stops the loop within the bound and hands back the reports channel.
    pub(super) fn stop(self) -> Receiver<MotionReport> {
        self.shared.request_stop();
        self.mailbox.stop();
        self.done
            .recv_timeout(BOUND)
            .expect("the render loop ends within the bound");
        self.join.join().expect("the render loop does not panic");
        self.reports
    }
}
