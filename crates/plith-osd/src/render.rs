//! The render thread: owns the device, the swap chain target, the painter and the model.
//!
//! At rest it blocks on the event channel until a message arrives or the model's next deadline
//! (the hide or the exit), so it parks with no wake-ups when nothing is due. While animating it
//! paces frames on the compositor clock and writes one `motion osd-show|osd-hide` line per
//! opacity fade. A device loss rebuilds the graphics objects (hardware, then WARP), and so does
//! a relayout after a display or DPI change, at the new size and scale. When the
//! model says exit, or on any failure, it posts [`WM_APP_EXIT`] to the window so the UI thread
//! winds down.

mod motion;
mod stack;

use crate::band;
use crate::log::Log;
use crate::model::OsdModel;
use motion::MotionTrack;
use plith_core::clock_choice::ClockKind;
use plith_core::layout::SurfaceLayout;
use plith_core::motion::{FramePhases, MotionKind, MotionStats};
use plith_ipc::Message;
use plith_render::clock::{open_clock, seconds_since};
use plith_render::{ClockError, FrameSource};
use stack::{Stack, is_device_lost};
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
    /// The client closed the pipe.
    Disconnected,
    /// The process that sent `Hello` with this pid has ended.
    ClientGone(u32),
    /// The display changed: the window now has this rectangle and scale.
    Relayout(SurfaceLayout),
    /// Stop now (the UI thread is closing).
    Quit,
}

/// Everything the render thread needs, all of it `Send`.
pub struct Job {
    /// The OSD window as an integer.
    pub hwnd: usize,
    /// Surface rectangle and DPI scale at start; [`Event::Relayout`] replaces it.
    pub layout: SurfaceLayout,
    /// Display refresh rate, for the motion statistics.
    pub hz: f64,
    /// Time zero of the clock and the model.
    pub origin: Instant,
    /// Messages, disconnects, client exits and the quit request.
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

/// Builds the graphics objects for `layout` on hardware, then on WARP; logs every failed try.
fn build_stack(job: &Job, layout: &SurfaceLayout) -> Result<Stack, String> {
    for warp in [false, true] {
        match Stack::build(hwnd_of(job), layout, warp) {
            Ok(stack) => return Ok(stack),
            Err(error) => job
                .log
                .line(&format!("osd device build (warp={warp}) failed: {error}")),
        }
    }
    Err(String::from("no device: hardware and WARP both failed"))
}

struct Engine<'a> {
    job: &'a Job,
    /// The layout the stack is built for.
    layout: SurfaceLayout,
    /// A relayout that arrived and is not applied yet.
    relayout: Option<SurfaceLayout>,
    stack: Option<Stack>,
    clock: Box<dyn FrameSource>,
    model: OsdModel,
    client: Option<u32>,
    track: MotionTrack,
    stats: Option<MotionStats>,
    /// A `Level` or `Hide` arrived since the last frame.
    retargeted: bool,
    shown: bool,
}

/// Runs the render thread until the model says exit, a `Quit` arrives or a call fails.
///
/// # Errors
/// Returns the failing call (device on hardware and WARP, clock or a non-loss draw error).
pub fn run(job: &Job) -> Result<(), String> {
    let _exit = ExitPost(job.hwnd);
    let stack = build_stack(job, &job.layout)?;
    let clock = open_clock(ClockKind::Dcomp, job.origin, 0, job.hz)
        .map_err(|e| format!("open_clock(dcomp): {e}"))?;
    job.log.line(&format!(
        "osd render: warp={} font={} hz={}",
        stack.is_warp(),
        stack.family(),
        job.hz
    ));
    let mut engine = Engine {
        job,
        layout: job.layout,
        relayout: None,
        stack: Some(stack),
        clock,
        model: OsdModel::new(),
        client: None,
        track: MotionTrack::default(),
        stats: None,
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
            self.apply_relayout()?;
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
            self.job
                .log
                .line("osd exit: bye, client gone, disconnected or no hello");
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
            Event::Disconnected => self.model.disconnected(now),
            Event::ClientGone(pid) if self.client == Some(pid) => {
                self.client = None;
                self.job.log.line(&format!("osd client {pid} gone"));
                self.model.client_gone(now);
            }
            Event::ClientGone(_) => {}
            Event::Relayout(layout) => self.relayout = Some(layout),
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

    /// Ends the running motion, if any, and logs it.
    fn finish_motion(&mut self) {
        self.track.end();
        self.log_motion();
    }

    /// Logs the recorded motion, if any.
    fn log_motion(&mut self) {
        if let Some(stats) = self.stats.take() {
            let report = stats.finish();
            self.job.log.line(&report.to_string());
            for line in report.detail_lines() {
                self.job.log.line(&line);
            }
        }
    }

    /// One compositor tick. A timeout (no tick within 100 ms, for example while the display
    /// sleeps) counts as a tick at the current time. Returns the tick and the wait in ms.
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
        loop {
            let (now, wait_ms) = self.tick()?;
            if self.drain(now) == Flow::Stop || self.exit_due(now) {
                return Ok(Flow::Stop);
            }
            self.apply_relayout()?;
            let animating = self.model.step(now);
            let fading = self.model.is_fading();
            let decision = self.track.frame(self.kind(), fading);
            if decision.end_before {
                self.log_motion();
            }
            if let Some(kind) = decision.start {
                let mut stats = MotionStats::new(kind, self.job.hz, ClockKind::Dcomp.as_str());
                stats.note_first_wait(wait_ms);
                self.stats = Some(stats);
            }
            let (draw_ms, present_ms) = self.paint()?;
            let retargeted = std::mem::take(&mut self.retargeted);
            if decision.record
                && let Some(stats) = self.stats.as_mut()
            {
                if retargeted && decision.start.is_none() {
                    stats.note_retarget();
                }
                let phases = FramePhases {
                    wait_ms,
                    late_ms: 0.0,
                    draw_ms,
                    present_ms,
                };
                stats.record_frame(now, phases);
                if fading {
                    stats.note_visible(now);
                }
            }
            if decision.end_after {
                self.log_motion();
            }
            self.sync_visibility();
            if !animating {
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

    /// Rebuilds the graphics objects at a pending relayout's size and scale (the device-loss
    /// path) and repaints when the OSD is on screen. Nothing to do without a pending relayout.
    fn apply_relayout(&mut self) -> Result<(), String> {
        let Some(layout) = self.relayout.take() else {
            return Ok(());
        };
        self.layout = layout;
        self.stack = None;
        let stack = self.stack.insert(build_stack(self.job, &self.layout)?);
        self.job.log.line(&format!(
            "osd relayout rebuilt: warp={} {}x{} scale={}",
            stack.is_warp(),
            layout.surface.width,
            layout.surface.height,
            layout.scale
        ));
        if !self.model.is_hidden() {
            self.paint()?;
        }
        Ok(())
    }

    /// Draws and presents the model's frame. On a device loss rebuilds everything once
    /// (hardware, then WARP) and draws again. Returns draw and present times in ms.
    fn paint(&mut self) -> Result<(f64, f64), String> {
        let frame = self.model.frame();
        let first = match self.stack.as_mut() {
            Some(stack) => stack.paint(&frame),
            None => return Err(String::from("no device")),
        };
        match first {
            Ok(times) => Ok(times),
            Err(error) if is_device_lost(error.code()) => {
                self.job
                    .log
                    .line(&format!("osd device lost ({error}), rebuilding"));
                self.stack = None;
                let stack = self.stack.insert(build_stack(self.job, &self.layout)?);
                self.job
                    .log
                    .line(&format!("osd device rebuilt: warp={}", stack.is_warp()));
                stack
                    .paint(&frame)
                    .map_err(|e| format!("paint after rebuild: {e}"))
            }
            Err(error) => Err(format!("paint: {error}")),
        }
    }
}
