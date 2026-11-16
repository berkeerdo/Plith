//! The render thread: owns the device, the swap chain target, the painter and the model.
//!
//! At rest it blocks on the event channel until a message arrives or the model's next deadline
//! (the hide or the exit), so it parks with no wake-ups when nothing is due. While animating it
//! paces frames on the compositor clock and writes one `motion osd-show|osd-hide` line per
//! motion. When the model says exit, or on any failure, it posts [`WM_APP_EXIT`] to the window
//! so the UI thread winds down.

use crate::band;
use crate::log::Log;
use crate::model::OsdModel;
use crate::paint::Painter;
use plith_core::clock_choice::ClockKind;
use plith_core::layout::SurfaceLayout;
use plith_core::levers::Levers;
use plith_core::motion::{FramePhases, MotionKind, MotionStats};
use plith_ipc::Message;
use plith_render::clock::{open_clock, seconds_since};
use plith_render::{ClockError, CompositionTarget, FrameSource, Gpu};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

/// Posted to the window when the render thread ends; the UI thread then stops.
pub const WM_APP_EXIT: u32 = WM_APP + 1;

/// What reaches the render thread.
#[derive(Debug)]
pub enum Event {
    /// A decoded protocol message.
    Message(Message),
    /// The process that sent `Hello` with this pid has ended.
    ClientGone(u32),
    /// Stop now (the UI thread is closing).
    Quit,
}

/// Everything the render thread needs, all of it `Send`.
pub struct Job {
    /// The OSD window as an integer.
    pub hwnd: usize,
    /// Surface rectangle and DPI scale.
    pub layout: SurfaceLayout,
    /// Display refresh rate, for the motion statistics.
    pub hz: f64,
    /// Time zero of the clock and the model.
    pub origin: Instant,
    /// Messages, client exits and the quit request.
    pub events: Receiver<Event>,
    /// The log.
    pub log: Arc<Log>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Stop,
}

/// Posts [`WM_APP_EXIT`] when dropped, so a failure or panic also ends the UI thread.
struct ExitPost(usize);

impl Drop for ExitPost {
    fn drop(&mut self) {
        let hwnd = HWND(self.0 as *mut core::ffi::c_void);
        // SAFETY: posting to a window handle never dereferences it; a dead window just fails.
        let _ = unsafe { PostMessageW(Some(hwnd), WM_APP_EXIT, WPARAM(0), LPARAM(0)) };
    }
}

fn hwnd_of(job: &Job) -> HWND {
    HWND(job.hwnd as *mut core::ffi::c_void)
}

struct Engine<'a> {
    job: &'a Job,
    gpu: Gpu,
    target: CompositionTarget,
    painter: Painter,
    clock: Box<dyn FrameSource>,
    model: OsdModel,
    client: Option<u32>,
    motion: Option<(MotionKind, MotionStats)>,
    /// A `Level` or `Hide` arrived since the last frame.
    retargeted: bool,
    shown: bool,
}

/// Runs the render thread until the model says exit, a `Quit` arrives or a call fails.
///
/// # Errors
/// Returns the failing call (device, target, painter, clock or present).
pub fn run(job: &Job) -> Result<(), String> {
    let _exit = ExitPost(job.hwnd);
    let gpu = Gpu::new(false).map_err(|e| format!("Gpu::new: {e}"))?;
    let target = CompositionTarget::new(&gpu, hwnd_of(job), &job.layout, Levers::default())
        .map_err(|e| format!("CompositionTarget::new: {e}"))?;
    let painter = Painter::new(&gpu).map_err(|e| format!("painter: {e}"))?;
    let clock = open_clock(ClockKind::Dcomp, job.origin, 0, job.hz)
        .map_err(|e| format!("open_clock(dcomp): {e}"))?;
    job.log.line(&format!(
        "osd render: warp={} font={} hz={}",
        gpu.is_warp(),
        painter.family(),
        job.hz
    ));
    let mut engine = Engine {
        job,
        gpu,
        target,
        painter,
        clock,
        model: OsdModel::new(),
        client: None,
        motion: None,
        retargeted: false,
        shown: false,
    };
    let result = engine.run();
    engine.finish_motion();
    result
}

impl Engine<'_> {
    fn now(&self) -> f64 {
        seconds_since(self.job.origin)
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            if self.rest() == Flow::Stop {
                return Ok(());
            }
            let now = self.now();
            if self.exit_due(now) {
                return Ok(());
            }
            if self.model.step(now) && self.animate()? == Flow::Stop {
                return Ok(());
            }
        }
    }

    fn exit_due(&self, now: f64) -> bool {
        let due = self.model.should_exit(now);
        if due {
            self.job.log.line("osd exit: client said bye or went away");
        }
        due
    }

    /// Blocks until an event or the model's deadline, then takes every pending event.
    fn rest(&mut self) -> Flow {
        let first = match self.model.deadline() {
            Some(at) => {
                let left = (at - self.now()).max(0.0);
                self.job.events.recv_timeout(Duration::from_secs_f64(left))
            }
            None => self
                .job
                .events
                .recv()
                .map_err(|_| RecvTimeoutError::Disconnected),
        };
        match first {
            Ok(event) => {
                if self.handle(event, self.now()) == Flow::Stop {
                    return Flow::Stop;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Flow::Stop,
        }
        self.drain(self.now())
    }

    /// Applies every pending event without blocking.
    fn drain(&mut self, now: f64) -> Flow {
        loop {
            match self.job.events.try_recv() {
                Ok(event) => {
                    if self.handle(event, now) == Flow::Stop {
                        return Flow::Stop;
                    }
                }
                Err(TryRecvError::Empty) => return Flow::Continue,
                Err(TryRecvError::Disconnected) => return Flow::Stop,
            }
        }
    }

    fn handle(&mut self, event: Event, now: f64) -> Flow {
        match event {
            Event::Message(message) => {
                match message {
                    Message::Hello { client_pid } => self.client = Some(client_pid),
                    Message::Level { .. } | Message::Hide => self.retargeted = true,
                    Message::Bye => {}
                }
                self.model.apply(&message, now);
            }
            Event::ClientGone(pid) if self.client == Some(pid) => {
                self.client = None;
                self.job.log.line(&format!("osd client {pid} gone"));
                self.model.client_gone(now);
            }
            Event::ClientGone(_) => {}
            Event::Quit => return Flow::Stop,
        }
        Flow::Continue
    }

    fn kind(&self) -> MotionKind {
        if self.model.is_showing() {
            MotionKind::OsdShow
        } else {
            MotionKind::OsdHide
        }
    }

    /// Starts a motion of the current kind, or ends the running one when the kind changed.
    /// A new target of the same kind counts as a retarget.
    fn track_motion(&mut self) {
        let kind = self.kind();
        let retargeted = std::mem::take(&mut self.retargeted);
        match self.motion.as_mut() {
            Some((running, stats)) if *running == kind => {
                if retargeted {
                    stats.note_retarget();
                }
            }
            _ => {
                self.finish_motion();
                let stats = MotionStats::new(kind, self.job.hz, ClockKind::Dcomp.as_str());
                self.motion = Some((kind, stats));
            }
        }
    }

    /// Logs the running motion, if any.
    fn finish_motion(&mut self) {
        if let Some((_, stats)) = self.motion.take() {
            let report = stats.finish();
            self.job.log.line(&report.to_string());
            for line in report.detail_lines() {
                self.job.log.line(&line);
            }
        }
    }

    /// One compositor tick. A timeout (no tick within 100 ms, for example while the display
    /// sleeps) counts as a tick at the current time.
    fn tick(&mut self) -> Result<(f64, f64), String> {
        let began = self.now();
        let now = match self.clock.wait() {
            Ok(now) => now,
            Err(ClockError::Timeout) => self.now(),
            Err(error) => return Err(format!("clock wait: {error}")),
        };
        Ok((now, (now - began) * 1000.0))
    }

    /// Clock-paced frames until the model rests. Returns `Stop` when the model says exit or
    /// a `Quit` arrives.
    fn animate(&mut self) -> Result<Flow, String> {
        self.retargeted = false;
        self.track_motion();
        let mut first = true;
        loop {
            let (now, wait_ms) = self.tick()?;
            if self.drain(now) == Flow::Stop || self.exit_due(now) {
                return Ok(Flow::Stop);
            }
            let animating = self.model.step(now);
            self.track_motion();
            let (draw_ms, present_ms) = self.paint()?;
            if let Some((_, stats)) = self.motion.as_mut() {
                if first {
                    stats.note_first_wait(wait_ms);
                    first = false;
                }
                let phases = FramePhases {
                    wait_ms,
                    late_ms: 0.0,
                    draw_ms,
                    present_ms,
                };
                stats.record_frame(now, phases);
                if animating {
                    stats.note_visible(now);
                }
            }
            self.sync_visibility();
            if !animating {
                self.finish_motion();
                return Ok(Flow::Continue);
            }
        }
    }

    /// Shows the window once something is drawn and hides it once nothing is, so an idle OSD
    /// covers no part of a game.
    fn sync_visibility(&mut self) {
        let want = !self.model.is_hidden();
        if want != self.shown {
            band::set_visible(hwnd_of(self.job), want);
            self.shown = want;
        }
    }

    /// Draws the model's frame and presents it. Returns draw and present times in ms.
    fn paint(&mut self) -> Result<(f64, f64), String> {
        let frame = self.model.frame();
        let start = Instant::now();
        // SAFETY: BeginDraw on the live context; EndDraw follows on every path.
        unsafe { self.gpu.context().BeginDraw() };
        self.painter.draw(&self.gpu, &frame);
        // SAFETY: closes the BeginDraw above.
        unsafe { self.gpu.context().EndDraw(None, None) }.map_err(|e| format!("EndDraw: {e}"))?;
        let drawn = Instant::now();
        self.target.present().map_err(|e| format!("Present: {e}"))?;
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        Ok((ms(drawn - start), ms(drawn.elapsed())))
    }
}
