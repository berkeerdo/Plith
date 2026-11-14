use super::*;
use crate::clock::{ClockError, FrameSource};
use plith_core::clock_choice::ClockKind;
use plith_core::geometry::IslandShape;
use plith_core::island::IslandMotion;
use plith_core::layout::RectPx;
use plith_core::motion::{MotionKind, MotionReport};
use plith_core::scene::{Scene, SceneMailbox, ShapeFrame, Suppression};
use plith_core::shapes::{PAGES, REST};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::E_FAIL;

const PERIOD: f64 = 1.0 / 240.0;
const BOUND: Duration = Duration::from_millis(300);
const QUIET: Duration = Duration::from_millis(50);
const KINDS: [ClockKind; 3] = [ClockKind::Dcomp, ClockKind::Dxgi, ClockKind::Timer];

fn index(kind: ClockKind) -> usize {
    match kind {
        ClockKind::Dcomp => 0,
        ClockKind::Dxgi => 1,
        ClockKind::Timer => 2,
    }
}

/// Shared fake time, per-kind call counters and scenes published at given call numbers.
struct Harness {
    time: Mutex<f64>,
    total: AtomicU64,
    calls: [AtomicU64; 3],
    script: Vec<(u64, Scene)>,
    mailbox: Arc<SceneMailbox>,
    delay: Option<Duration>,
}

impl Harness {
    fn now(&self) -> f64 {
        *self.time.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn advance(&self, secs: f64) {
        *self.time.lock().unwrap_or_else(PoisonError::into_inner) += secs;
    }

    fn total(&self) -> u64 {
        self.total.load(Ordering::SeqCst)
    }

    fn calls(&self, kind: ClockKind) -> u64 {
        self.calls[index(kind)].load(Ordering::SeqCst)
    }
}

/// Returns `k / 240` on the k-th call, or the unchanged time when `instant`.
struct FakeClock {
    kind: ClockKind,
    instant: bool,
    harness: Arc<Harness>,
}

impl FrameSource for FakeClock {
    fn wait(&mut self) -> Result<f64, ClockError> {
        if let Some(delay) = self.harness.delay {
            thread::sleep(delay);
        }
        if !self.instant {
            self.harness.advance(PERIOD);
        }
        let call = self.harness.total.fetch_add(1, Ordering::SeqCst) + 1;
        self.harness.calls[index(self.kind)].fetch_add(1, Ordering::SeqCst);
        for (at, scene) in &self.harness.script {
            if *at == call {
                self.harness.mailbox.publish(*scene);
            }
        }
        Ok(self.harness.now())
    }

    fn kind(&self) -> ClockKind {
        self.kind
    }
}

/// Records painted frames; fails the first `fail` paints; records every `recover` call.
struct FakePaint {
    frames: Arc<Mutex<Vec<PaintFrame>>>,
    fail: Arc<AtomicU32>,
    recovers: Arc<Mutex<Vec<bool>>>,
    recover_fails: bool,
}

impl Paint for FakePaint {
    fn paint(&mut self, frame: &PaintFrame) -> windows::core::Result<FrameTimes> {
        let left = self.fail.load(Ordering::SeqCst);
        if left > 0 {
            self.fail.store(left - 1, Ordering::SeqCst);
            return Err(E_FAIL.into());
        }
        lock(&self.frames).push(frame.clone());
        Ok(FrameTimes {
            draw_ms: 1.0,
            present_ms: 0.5,
        })
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

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct Setup {
    instant: Vec<ClockKind>,
    fail: u32,
    recover_fails: bool,
    script: Vec<(u64, Scene)>,
    delay: Option<Duration>,
}

struct Run {
    mailbox: Arc<SceneMailbox>,
    shared: Arc<LoopShared>,
    harness: Arc<Harness>,
    frames: Arc<Mutex<Vec<PaintFrame>>>,
    shapes: Arc<Mutex<Vec<ShapeFrame>>>,
    recovers: Arc<Mutex<Vec<bool>>>,
    reports: Receiver<MotionReport>,
    fatal: Receiver<String>,
    done: Receiver<()>,
    join: JoinHandle<()>,
}

fn layout() -> SurfaceLayout {
    let monitor = RectPx {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    SurfaceLayout::new(monitor, 0, 1.0)
}

fn sinks(
    shapes: Arc<Mutex<Vec<ShapeFrame>>>,
) -> (RenderSinks, Receiver<MotionReport>, Receiver<String>) {
    let (report_tx, reports) = mpsc::channel();
    let (fatal_tx, fatal) = mpsc::channel();
    let sinks = RenderSinks {
        on_shape: Box::new(move |frame| lock(&shapes).push(frame)),
        on_motion: Box::new(move |report| {
            let _ = report_tx.send(report);
        }),
        on_fatal: Box::new(move |message| {
            let _ = fatal_tx.send(message);
        }),
    };
    (sinks, reports, fatal)
}

fn start(setup: Setup) -> Run {
    let mailbox = Arc::new(SceneMailbox::new(Scene::default()));
    let shared = Arc::new(LoopShared::new());
    let harness = Arc::new(Harness {
        time: Mutex::new(0.0),
        total: AtomicU64::new(0),
        calls: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
        script: setup.script,
        mailbox: Arc::clone(&mailbox),
        delay: setup.delay,
    });
    let frames = Arc::new(Mutex::new(Vec::new()));
    let shapes = Arc::new(Mutex::new(Vec::new()));
    let recovers = Arc::new(Mutex::new(Vec::new()));
    let (sinks, reports, fatal) = sinks(Arc::clone(&shapes));
    let paint = FakePaint {
        frames: Arc::clone(&frames),
        fail: Arc::new(AtomicU32::new(setup.fail)),
        recovers: Arc::clone(&recovers),
        recover_fails: setup.recover_fails,
    };
    let (done_tx, done) = mpsc::channel();
    let join = {
        let (mailbox, shared, harness) = (
            Arc::clone(&mailbox),
            Arc::clone(&shared),
            Arc::clone(&harness),
        );
        let instant = setup.instant;
        thread::spawn(move || {
            let clock_harness = Arc::clone(&harness);
            let parts = LoopParts {
                paint,
                open_clock: move |kind| {
                    Ok::<_, ClockError>(FakeClock {
                        kind,
                        instant: instant.contains(&kind),
                        harness: Arc::clone(&clock_harness),
                    })
                },
                now: move || harness.now(),
                order: KINDS.to_vec(),
                hz: 240.0,
                layout: layout(),
            };
            run_loop(parts, &mailbox, &shared, &sinks);
            let _ = done_tx.send(());
        })
    };
    Run {
        mailbox,
        shared,
        harness,
        frames,
        shapes,
        recovers,
        reports,
        fatal,
        done,
        join,
    }
}

impl Run {
    fn frames(&self) -> Vec<PaintFrame> {
        lock(&self.frames).clone()
    }

    fn report(&self) -> MotionReport {
        self.reports
            .recv_timeout(BOUND)
            .expect("a motion report within the bound")
    }

    fn wait_frames(&self, n: usize) {
        let deadline = Instant::now() + BOUND;
        while lock(&self.frames).len() < n {
            assert!(Instant::now() < deadline, "expected {n} painted frames");
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn wait_calls(&self, n: u64) {
        let deadline = Instant::now() + BOUND;
        while self.harness.total() < n {
            assert!(Instant::now() < deadline, "expected {n} clock waits");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Asserts that no clock wait happens for a while: the thread is parked.
    fn assert_parked(&self) {
        let before = self.harness.total();
        thread::sleep(QUIET);
        assert_eq!(self.harness.total(), before, "clock waits while parked");
    }

    fn wakes(&self) -> u64 {
        self.shared.wakes()
    }

    /// Stops the loop within the bound and hands back the reports channel.
    fn stop(self) -> Receiver<MotionReport> {
        self.shared.request_stop();
        self.mailbox.stop();
        self.done
            .recv_timeout(BOUND)
            .expect("the render loop ends within the bound");
        self.join.join().expect("the render loop does not panic");
        self.reports
    }
}

fn expanded(page: usize) -> Scene {
    Scene {
        expanded: true,
        page,
        suppression: Suppression::default(),
    }
}

fn one_period_from(from: IslandShape, to: IslandShape) -> IslandShape {
    let mut motion = IslandMotion::new(from);
    motion.retarget(to);
    motion.step(PERIOD);
    motion.shape()
}

#[test]
fn parks_at_rest_with_no_clock_waits() {
    let run = start(Setup::default());
    run.wait_frames(1);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(run.harness.total(), 0);
    assert_eq!(run.wakes(), 0);
    let frames = run.frames();
    assert_eq!(frames.len(), 1, "exactly one rest frame at start");
    assert_eq!(frames[0].shape, REST);
    assert!(!frames[0].clip);
    assert!(
        run.reports.try_recv().is_err(),
        "the start frame is no motion"
    );
    let _ = run.stop();
}

#[test]
fn suppressed_scene_parks_even_mid_motion() {
    let suppressed = Scene {
        suppression: Suppression {
            fullscreen: true,
            ..Suppression::default()
        },
        ..expanded(0)
    };
    let run = start(Setup {
        script: vec![(20, suppressed)],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(20);
    run.assert_parked();
    assert_eq!(
        run.harness.total(),
        20,
        "no clock wait after the suppression"
    );
    assert!(
        run.reports.try_recv().is_err(),
        "the motion report is dropped"
    );
    assert_eq!(
        run.frames().len(),
        1 + 19,
        "the suppressed frame is not painted"
    );
    assert_eq!(run.wakes(), 1 + 20);
    let _ = run.stop();
}

#[test]
fn unsuppress_paints_one_frame_without_clock_waits() {
    let suppressed = Scene {
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
        ..expanded(0)
    };
    let run = start(Setup {
        script: vec![(20, suppressed)],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(20);
    run.assert_parked();
    run.mailbox.publish(expanded(0));
    run.wait_frames(21);
    run.assert_parked();
    let frames = run.frames();
    assert_eq!(frames.len(), 21, "exactly one frame for the snapped target");
    assert_eq!(
        frames[20].shape, PAGES[0],
        "shows the target with no catch-up"
    );
    assert_eq!(run.harness.total(), 20);
    assert_eq!(run.wakes(), 1 + 20 + 1);
    assert!(run.reports.try_recv().is_err());
    let _ = run.stop();
}

#[test]
fn one_expand_produces_one_motion_report() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Expand);
    assert!(report.frames > 100, "frames {}", report.frames);
    assert_eq!(report.retargets, 0);
    assert_eq!(report.clock, "dcomp");
    assert_eq!(report.hitches, 0);
    run.assert_parked();
    let calls = run.harness.total();
    assert_eq!(u64::try_from(report.frames).ok(), Some(calls));
    assert_eq!(run.wakes(), 1 + calls);
    let frames = run.frames();
    assert_eq!(frames.len(), 1 + report.frames);
    let last = frames.last().expect("frames");
    assert_eq!(last.shape, PAGES[0]);
    assert!(!last.clip, "the settled last frame is not clipped");
    assert!(run.reports.recv_timeout(QUIET).is_err(), "only one report");
    let _ = run.stop();
}

#[test]
fn retarget_mid_motion_is_one_report_with_retargets_1() {
    let run = start(Setup {
        script: vec![(30, expanded(1))],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.kind, MotionKind::Expand);
    assert_eq!(report.retargets, 1);
    run.assert_parked();
    assert!(run.reports.recv_timeout(QUIET).is_err(), "only one report");
    assert_eq!(run.frames().last().map(|f| f.shape), Some(PAGES[1]));
    let _ = run.stop();
}

#[test]
fn first_frame_after_rest_advances_one_period() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let expand = run.report();
    let frames = run.frames();
    assert_eq!(frames[1].shape, one_period_from(REST, PAGES[0]));
    assert!(frames[1].clip);
    assert_eq!(frames[1].origin, layout().island_origin(frames[1].shape));
    run.assert_parked();
    // A rest gap of 12 periods: without a timeline and stamper reset the first step would be 13.
    run.harness.advance(12.0 * PERIOD);
    run.mailbox.publish(Scene::default());
    let collapse = run.report();
    assert_eq!(collapse.kind, MotionKind::Collapse);
    let first = &run.frames()[1 + expand.frames];
    assert_eq!(first.shape, one_period_from(PAGES[0], REST));
    let _ = run.stop();
}

#[test]
fn on_shape_is_called_every_painted_frame() {
    let run = start(Setup::default());
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let _ = run.report();
    run.assert_parked();
    let frames = run.frames();
    let shapes = lock(&run.shapes).clone();
    assert_eq!(shapes.len(), frames.len());
    for (i, (shape, frame)) in shapes.iter().zip(&frames).enumerate() {
        assert_eq!(shape.shape, frame.shape, "frame {i}");
        assert_eq!(shape.generation, u64::try_from(i + 1).ok().unwrap_or(0));
    }
    let _ = run.stop();
}

#[test]
fn three_fast_returns_demote_the_clock() {
    let run = start(Setup {
        instant: vec![ClockKind::Dcomp],
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(report.clock, "dxgi");
    assert_eq!(run.harness.calls(ClockKind::Dcomp), 3);
    assert_eq!(run.harness.calls(ClockKind::Timer), 0);
    run.assert_parked();
    assert_eq!(run.wakes(), 1 + run.harness.total());
    // The demoted clock stays demoted for the next motion.
    run.mailbox.publish(Scene::default());
    assert_eq!(run.report().clock, "dxgi");
    assert_eq!(run.harness.calls(ClockKind::Dcomp), 3);
    let _ = run.stop();
}

#[test]
fn failed_paints_recreate_then_fall_back_to_warp() {
    let run = start(Setup {
        fail: 3,
        ..Setup::default()
    });
    let deadline = Instant::now() + BOUND;
    while lock(&run.recovers).is_empty() {
        assert!(Instant::now() < deadline, "the failed start frame recovers");
        thread::sleep(Duration::from_millis(1));
    }
    run.mailbox.publish(expanded(0));
    let report = run.report();
    assert_eq!(*lock(&run.recovers), vec![false, false, true]);
    assert_eq!(
        report.frames + 2,
        usize::try_from(run.harness.total()).unwrap_or(0)
    );
    assert_eq!(run.frames().len(), report.frames);
    let _ = run.stop();
}

#[test]
fn failed_recover_ends_the_thread_with_a_fatal_message() {
    let run = start(Setup {
        fail: 1,
        recover_fails: true,
        ..Setup::default()
    });
    let message = run.fatal.recv_timeout(BOUND).expect("a fatal message");
    assert!(!message.is_empty());
    run.done
        .recv_timeout(BOUND)
        .expect("the loop ends by itself");
    assert_eq!(run.harness.total(), 0);
}

#[test]
fn stop_mid_motion_ends_the_loop_without_a_report() {
    let run = start(Setup {
        delay: Some(Duration::from_millis(1)),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(5);
    let reports = run.stop();
    assert!(
        reports.try_recv().is_err(),
        "a stopped motion is not reported"
    );
}
