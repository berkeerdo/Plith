//! The band spike: does a DirectComposition swap chain show in a window moved into the
//! UIAccess band? The UI thread owns the window and pumps its messages; a render thread owns
//! every graphics object and presents on the compositor clock.

use crate::band::{self, BandWindow, Creation};
use crate::log::Log;
use plith_core::clock_choice::ClockKind;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_core::levers::Levers;
use plith_render::clock::open_clock;
use plith_render::{CompositionTarget, Gpu};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};

/// Window size in DIP.
const WIDTH_DIP: f64 = 360.0;
/// Window height in DIP.
const HEIGHT_DIP: f64 = 96.0;
/// How often the UI thread looks for messages and for the render thread's end.
const PUMP_SLEEP: Duration = Duration::from_millis(4);

/// Frame timings of one second, summarised as a `motion` line.
#[derive(Debug, Default)]
pub struct Second {
    gaps_ms: Vec<f64>,
    present_ms: Vec<f64>,
}

/// The value at the 99th percentile (nearest rank), 0 for no samples.
#[must_use]
pub fn p99(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let rank = (0.99 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

impl Second {
    /// Records one frame: the gap since the previous tick and how long `Present` took.
    pub fn push(&mut self, gap_ms: f64, present_ms: f64) {
        self.gaps_ms.push(gap_ms);
        self.present_ms.push(present_ms);
    }

    /// `motion spike <mode>: N frames (F fps), worst gap G ms, present p99 P ms` over
    /// `seconds` of wall time.
    #[must_use]
    pub fn summary(&self, mode: &str, seconds: f64) -> String {
        let frames = self.gaps_ms.len();
        #[allow(clippy::cast_precision_loss)]
        let fps = if seconds > 0.0 {
            frames as f64 / seconds
        } else {
            0.0
        };
        let worst = self.gaps_ms.iter().copied().fold(0.0_f64, f64::max);
        format!(
            "motion spike {mode}: {frames} frames ({fps:.1} fps), worst gap {worst:.2} ms, \
             present p99 {:.2} ms",
            p99(&self.present_ms)
        )
    }
}

/// What the render thread needs, all of it `Send`.
struct Job {
    hwnd: usize,
    layout: SurfaceLayout,
    mode: Creation,
    seconds: u32,
    head: String,
    log: Arc<Log>,
}

fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

/// Draws the pill and a level bar filled to `level` (0 to 1), in DIP.
fn draw(gpu: &Gpu, pill: &ID2D1SolidColorBrush, bar: &ID2D1SolidColorBrush, level: f32) {
    #[allow(clippy::cast_possible_truncation)]
    let (w, h) = (WIDTH_DIP as f32, HEIGHT_DIP as f32);
    let shape = D2D1_ROUNDED_RECT {
        rect: rect(8.0, 8.0, w - 8.0, h - 8.0),
        radiusX: (h - 16.0) / 2.0,
        radiusY: (h - 16.0) / 2.0,
    };
    let track = D2D1_ROUNDED_RECT {
        rect: rect(
            48.0,
            h / 2.0 - 4.0,
            48.0 + (w - 96.0) * level,
            h / 2.0 + 4.0,
        ),
        radiusX: 4.0,
        radiusY: 4.0,
    };
    let clear = color(0.0, 0.0, 0.0, 0.0);
    let ctx = gpu.context();
    // SAFETY: the context has the swap chain back buffer as target; the structs are live.
    unsafe {
        ctx.Clear(Some(&raw const clear));
        ctx.FillRoundedRectangle(&raw const shape, pill);
        ctx.FillRoundedRectangle(&raw const track, bar);
    }
}

fn brushes(gpu: &Gpu) -> windows::core::Result<(ID2D1SolidColorBrush, ID2D1SolidColorBrush)> {
    let (dark, light) = (color(0.08, 0.08, 0.09, 0.92), color(0.95, 0.95, 0.97, 1.0));
    let ctx = gpu.context();
    // SAFETY: the colors are live locals for the calls.
    unsafe {
        Ok((
            ctx.CreateSolidColorBrush(&raw const dark, None)?,
            ctx.CreateSolidColorBrush(&raw const light, None)?,
        ))
    }
}

/// The render thread: device, target, then one frame per compositor tick until time is up.
fn render(job: &Job) -> Result<(), String> {
    let gpu = Gpu::new(false).map_err(|e| format!("Gpu::new: {e}"))?;
    let hwnd = HWND(job.hwnd as *mut core::ffi::c_void);
    let mut target = CompositionTarget::new(&gpu, hwnd, &job.layout, Levers::default())
        .map_err(|e| format!("CompositionTarget::new: {e}"))?;
    let (pill, bar) = brushes(&gpu).map_err(|e| format!("CreateSolidColorBrush: {e}"))?;
    job.log
        .line(&format!("spike {} warp={}", job.head, gpu.is_warp()));
    let origin = Instant::now();
    let mut clock = open_clock(ClockKind::Dcomp, origin, 0, 60.0)
        .map_err(|e| format!("open_clock(dcomp): {e}"))?;
    let (mut second, mut second_start, mut last) = (Second::default(), 0.0, 0.0);
    let mode = job.mode.as_str();
    while origin.elapsed().as_secs_f64() < f64::from(job.seconds) {
        let now = clock.wait().map_err(|e| format!("clock wait: {e}"))?;
        #[allow(clippy::cast_possible_truncation)]
        let level = now.fract() as f32;
        // SAFETY: BeginDraw on the live context; EndDraw follows on every path.
        unsafe { gpu.context().BeginDraw() };
        draw(&gpu, &pill, &bar, level);
        // SAFETY: closes the BeginDraw above.
        unsafe { gpu.context().EndDraw(None, None) }.map_err(|e| format!("EndDraw: {e}"))?;
        let before = Instant::now();
        target.present().map_err(|e| format!("Present: {e}"))?;
        let present_ms = before.elapsed().as_secs_f64() * 1000.0;
        if last > 0.0 {
            second.push((now - last) * 1000.0, present_ms);
        }
        last = now;
        if now - second_start >= 1.0 {
            job.log.line(&second.summary(mode, now - second_start));
            (second, second_start) = (Second::default(), now);
        }
    }
    Ok(())
}

/// Pumps this thread's messages until `worker` ends.
fn pump_until(worker: &thread::JoinHandle<Result<(), String>>) {
    let mut msg = MSG::default();
    while !worker.is_finished() {
        // SAFETY: `msg` is a live local; messages go to windows of this thread.
        unsafe {
            while PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
            }
        }
        thread::sleep(PUMP_SLEEP);
    }
}

/// The token UIAccess flag as a log word, or the error that hid it.
fn uiaccess() -> String {
    band::token_ui_access().map_or_else(|e| e, |flag| flag.to_string())
}

fn head(window: &BandWindow, surface: RectPx, scale: f64) -> String {
    let uiaccess = uiaccess();
    let band = window
        .band
        .map_or_else(|| String::from("unknown"), |b| b.to_string());
    format!(
        "{}: band={band} immersive={} uiaccess={uiaccess} rect={},{} {}x{} scale={scale}",
        window.creation.as_str(),
        band::is_immersive(),
        surface.x,
        surface.y,
        surface.width,
        surface.height,
    )
}

/// Runs the spike for `seconds`: creates the window the `creation` way at the primary
/// monitor's top centre and draws on a render thread. Logs a `spike` line at start and a
/// `motion` line every second.
///
/// # Errors
/// Returns the first failure (window creation, a band call Windows refused, the device, the
/// clock or a present); the caller logs it and exits non-zero.
pub fn run(creation: Creation, seconds: u32, log: &Arc<Log>) -> Result<(), String> {
    let (monitor, scale) = band::primary_monitor()?;
    let surface = band::top_centre(monitor, scale, WIDTH_DIP, HEIGHT_DIP);
    let window = band::create_band_window(surface, creation).map_err(|e| {
        format!(
            "{e} (uiaccess={} immersive={})",
            uiaccess(),
            band::is_immersive()
        )
    })?;
    let job = Job {
        hwnd: window.hwnd.0 as usize,
        layout: SurfaceLayout { surface, scale },
        mode: creation,
        seconds,
        head: head(&window, surface, scale),
        log: Arc::clone(log),
    };
    let worker = thread::Builder::new()
        .name(String::from("plith-osd-render"))
        .spawn(move || render(&job))
        .map_err(|e| format!("spawn render thread: {e}"))?;
    pump_until(&worker);
    let result = worker
        .join()
        .unwrap_or_else(|_| Err(String::from("render thread panicked")));
    drop(window);
    result
}

#[cfg(test)]
mod tests;
