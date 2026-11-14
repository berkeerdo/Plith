//! The render thread: spawn, handle, configuration and sinks. Every graphics object is created
//! inside the thread; only `Arc`s, the sinks and plain integers (HWND, HMONITOR) cross.

mod clocks;
mod run;

pub use run::{LoopParts, LoopShared, run_loop};

use crate::clock::{open_clock, seconds_since};
use crate::gpu_paint::GpuPaint;
use crate::levers::{apply_mmcss, apply_sched_class};
use crate::paint::PaintFrame;
use plith_core::clock_choice::clock_order;
use plith_core::launch::ClockRequest;
use plith_core::layout::SurfaceLayout;
use plith_core::levers::{Levers, MmcssPriority, outcome_line};
use plith_core::motion::MotionReport;
use plith_core::scene::{SceneMailbox, ShapeFrame};
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
};
use windows::core::w;

/// Cost of one frame in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameTimes {
    /// Time spent drawing.
    pub draw_ms: f64,
    /// Time spent presenting.
    pub present_ms: f64,
}

/// Paints and presents frames; fallible so device loss reaches `GpuHealth`.
pub trait Paint {
    /// Draws and presents `frame`.
    ///
    /// # Errors
    /// Returns the failing draw or present call (for example device removed).
    fn paint(&mut self, frame: &PaintFrame) -> windows::core::Result<FrameTimes>;

    /// Rebuilds the device and target, on WARP when `warp` is set.
    ///
    /// # Errors
    /// Returns the failing creation call; the render thread then ends.
    fn recover(&mut self, warp: bool) -> windows::core::Result<()>;

    /// Log lines about levers applied since the last call (at start and after a rebuild).
    fn take_notes(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// Callbacks the render thread calls from its own thread.
pub struct RenderSinks {
    /// After every painted frame, with the drawn shape and a per-thread frame counter.
    pub on_shape: Box<dyn Fn(ShapeFrame) + Send>,
    /// Once per finished motion.
    pub on_motion: Box<dyn Fn(MotionReport) + Send>,
    /// Once, when the thread ends on an unrecoverable error.
    pub on_fatal: Box<dyn Fn(String) + Send>,
    /// A non-fatal log line from the render thread (for example a lever outcome).
    pub on_note: Box<dyn Fn(String) + Send>,
}

/// What the render thread needs to start.
#[derive(Debug, Clone, Copy)]
pub struct RenderConfig {
    /// The notch window's `HWND` as an integer.
    pub hwnd: isize,
    /// The notch monitor's `HMONITOR` as an integer.
    pub monitor: isize,
    /// The surface layout of the window.
    pub layout: SurfaceLayout,
    /// The requested frame clock.
    pub clock: ClockRequest,
    /// True in a remote session (timer clock only).
    pub remote: bool,
    /// Nominal refresh rate of the notch monitor.
    pub hz: f64,
    /// The load levers (the default is the R1 behaviour).
    pub levers: Levers,
}

/// Why the render thread did not start.
#[derive(Debug)]
pub enum RenderError {
    /// The OS refused to create the thread.
    Spawn(std::io::Error),
    /// The device or the composition target could not be created.
    Init(windows::core::Error),
    /// The thread ended before reporting its start.
    Exited,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "cannot start the render thread: {error}"),
            Self::Init(error) => write!(f, "cannot create the renderer: {error}"),
            Self::Exited => write!(f, "the render thread ended during start"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Owns the running render thread; dropping it stops and joins the thread.
pub struct RenderHandle {
    mailbox: Arc<SceneMailbox>,
    shared: Arc<LoopShared>,
    join: Option<JoinHandle<()>>,
}

/// Asks the render thread to stop without joining it; usable from any thread.
#[derive(Clone)]
pub struct StopHandle {
    mailbox: Arc<SceneMailbox>,
    shared: Arc<LoopShared>,
}

impl StopHandle {
    /// Requests the stop: a moving loop ends at its next frame, a parked one is released.
    pub fn request(&self) {
        self.shared.request_stop();
        self.mailbox.stop();
    }
}

impl RenderHandle {
    /// A handle that can request the stop while another owner keeps the join.
    #[must_use]
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle {
            mailbox: Arc::clone(&self.mailbox),
            shared: Arc::clone(&self.shared),
        }
    }

    /// Returns from any wait (mailbox or clock) so far; zero growth means parked.
    #[must_use]
    pub fn wake_count(&self) -> u64 {
        self.shared.wakes()
    }

    /// Stops the mailbox and joins the thread. A thread inside a clock wait ends after that
    /// wait returns.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for RenderHandle {
    fn drop(&mut self) {
        self.shared.request_stop();
        self.mailbox.stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Starts the `plith-render` thread and waits until its device and target exist.
///
/// # Errors
/// Returns [`RenderError`] when the thread or its renderer cannot start.
pub fn spawn(
    config: RenderConfig,
    mailbox: Arc<SceneMailbox>,
    sinks: RenderSinks,
) -> Result<RenderHandle, RenderError> {
    let shared = Arc::new(LoopShared::new());
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let join = {
        let (mailbox, shared) = (Arc::clone(&mailbox), Arc::clone(&shared));
        thread::Builder::new()
            .name(String::from("plith-render"))
            .spawn(move || render_main(config, &mailbox, &shared, &sinks, &ready_tx))
            .map_err(RenderError::Spawn)?
    };
    // The thread sends exactly once before its loop, or drops the sender when it ends early,
    // so this receive is finite. On failure the thread has already returned: join it without
    // stopping the caller's mailbox.
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(RenderHandle {
            mailbox,
            shared,
            join: Some(join),
        }),
        Ok(Err(error)) => {
            let _ = join.join();
            Err(RenderError::Init(error))
        }
        Err(_) => {
            let _ = join.join();
            Err(RenderError::Exited)
        }
    }
}

/// The thread body: MMCSS, renderer, loop, MMCSS revert.
fn render_main(
    config: RenderConfig,
    mailbox: &SceneMailbox,
    shared: &LoopShared,
    sinks: &RenderSinks,
    ready: &SyncSender<windows::core::Result<()>>,
) {
    let mmcss = join_mmcss(config.levers.mmcss, sinks);
    if let Some(outcome) = apply_sched_class(config.levers.sched_class) {
        (sinks.on_note)(outcome_line(&outcome));
    }
    let origin = Instant::now();
    let hwnd = HWND(config.hwnd as *mut core::ffi::c_void);
    match GpuPaint::new(hwnd, config.layout, false, config.levers) {
        Ok(mut paint) => {
            for line in paint.take_notes() {
                (sinks.on_note)(line);
            }
            let _ = ready.send(Ok(()));
            let parts = LoopParts {
                paint,
                open_clock: |kind| open_clock(kind, origin, config.monitor, config.hz),
                now: || seconds_since(origin),
                order: clock_order(config.clock, config.remote),
                hz: config.hz,
                layout: config.layout,
                dwm: crate::dwm::read,
            };
            run_loop(parts, mailbox, shared, sinks);
        }
        Err(error) => {
            let _ = ready.send(Err(error));
        }
    }
    if let Some(handle) = mmcss {
        // SAFETY: `handle` came from `AvSetMmThreadCharacteristicsW` on this thread.
        let _ = unsafe { AvRevertMmThreadCharacteristics(handle) };
    }
}

/// Joins the MMCSS "Games" task (failure is ignored) and applies the priority lever to it.
fn join_mmcss(priority: MmcssPriority, sinks: &RenderSinks) -> Option<HANDLE> {
    let mut index = 0_u32;
    // SAFETY: the task name is a static wide string; `index` is a live local.
    let task = unsafe { AvSetMmThreadCharacteristicsW(w!("Games"), &raw mut index) }.ok()?;
    if let Some(outcome) = apply_mmcss(task, priority) {
        (sinks.on_note)(outcome_line(&outcome));
    }
    Some(task)
}

#[cfg(test)]
mod tests;
