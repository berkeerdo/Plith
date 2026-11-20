//! The settings sheet renderer: a `plith-sheet` thread that lives while the settings window is
//! open and owns its own device, context, swap chain and text system, so the island's render
//! thread never waits on it. It paints only when a newer list or a new size arrives: no clock,
//! no loop at rest.

mod paint;
mod target;
#[cfg(test)]
mod tests;

use crate::device::Gpu;
use crate::thread::RenderError;
use paint::SheetPainter;
use plith_core::draw::DisplayList;
use plith_core::panic_text::payload_text;
use plith_core::theme::Palette;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use target::SheetSurface;
use windows::Win32::Foundation::HWND;

/// What the sheet thread needs to start.
#[derive(Debug, Clone, Copy)]
pub struct SheetConfig {
    /// The settings window's `HWND` as an integer.
    pub hwnd: isize,
    /// Client width in pixels.
    pub width_px: u32,
    /// Client height in pixels.
    pub height_px: u32,
    /// Pixels per DIP.
    pub scale: f64,
    /// The colours.
    pub palette: Palette,
}

/// A size in pixels and its DIP scale.
type Size = (u32, u32, f64);

/// What the owner handed over since the thread last looked: the newest only.
#[derive(Default)]
struct Inbox {
    list: Option<Arc<DisplayList>>,
    size: Option<Size>,
    stop: bool,
}

/// The inbox, its condvar and the counters tests read.
#[derive(Default)]
struct Shared {
    inbox: Mutex<Inbox>,
    bell: Condvar,
    paints: AtomicU64,
    wakes: AtomicU64,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inbox> {
        self.inbox.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Changes the inbox and wakes the thread.
    fn post(&self, change: impl FnOnce(&mut Inbox)) {
        change(&mut self.lock());
        self.bell.notify_one();
    }

    /// Blocks until something arrives; `None` on stop.
    fn wait(&self) -> Option<(Option<Arc<DisplayList>>, Option<Size>)> {
        let mut inbox = self.lock();
        while !inbox.stop && inbox.list.is_none() && inbox.size.is_none() {
            inbox = self
                .bell
                .wait(inbox)
                .unwrap_or_else(PoisonError::into_inner);
            self.wakes.fetch_add(1, Ordering::Relaxed);
        }
        if inbox.stop {
            return None;
        }
        Some((inbox.list.take(), inbox.size.take()))
    }
}

/// Owns the running sheet thread; dropping it stops and joins the thread.
pub struct SheetRender {
    shared: Arc<Shared>,
    join: Option<JoinHandle<()>>,
}

impl SheetRender {
    /// Starts the `plith-sheet` thread and waits until its device and swap chain exist.
    /// `on_fatal` is called once, from the sheet thread, when it ends on an error (a second
    /// device failure after one rebuild, or a panic).
    ///
    /// # Errors
    /// Returns [`RenderError`] when the thread or its renderer cannot start.
    pub fn spawn(
        config: SheetConfig,
        on_fatal: Box<dyn Fn(String) + Send>,
    ) -> Result<SheetRender, RenderError> {
        let shared = Arc::new(Shared::default());
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = {
            let shared = Arc::clone(&shared);
            thread::Builder::new()
                .name(String::from("plith-sheet"))
                .spawn(move || sheet_main(config, &shared, &*on_fatal, &ready_tx))
                .map_err(RenderError::Spawn)?
        };
        // The thread sends once before its loop, or drops the sender when it ends early.
        let started = match ready_rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(RenderError::Init(error)),
            Err(_) => Err(RenderError::Exited),
        };
        if let Err(error) = started {
            let _ = join.join();
            return Err(error);
        }
        Ok(SheetRender {
            shared,
            join: Some(join),
        })
    }

    /// Hands over a new list; a list not yet painted is replaced (latest wins).
    pub fn publish(&self, list: Arc<DisplayList>) {
        self.shared.post(|inbox| inbox.list = Some(list));
    }

    /// The client area is now `width_px` by `height_px` at `scale`; a zero size or a
    /// non-finite or non-positive scale is ignored.
    pub fn resize(&self, width_px: u32, height_px: u32, scale: f64) {
        self.shared
            .post(|inbox| inbox.size = Some((width_px, height_px, scale)));
    }

    /// Stops the thread and waits until it has ended.
    pub fn stop_and_join(self) {
        drop(self);
    }

    /// Frames painted so far.
    #[must_use]
    #[doc(hidden)]
    pub fn paints(&self) -> u64 {
        self.shared.paints.load(Ordering::Relaxed)
    }

    /// Returns from the thread's wait so far; no growth means it is parked.
    #[must_use]
    #[doc(hidden)]
    pub fn wakes(&self) -> u64 {
        self.shared.wakes.load(Ordering::Relaxed)
    }
}

impl Drop for SheetRender {
    fn drop(&mut self) {
        self.shared.post(|inbox| inbox.stop = true);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Draws `list` on WARP into a bitmap of the list's size at `scale` (the sheet is 840 x 640 DIP,
/// so scale 2 gives 1680 x 1280) on the opaque floor of `palette`, and returns tightly packed
/// premultiplied BGRA rows. For renders and tests; no window is involved.
///
/// # Errors
/// Returns [`RenderError::Init`] with the first failing Windows call.
pub fn render_sheet_offscreen(
    list: &DisplayList,
    scale: f64,
    palette: Palette,
) -> Result<Vec<u8>, RenderError> {
    paint::offscreen(list, scale, palette).map_err(RenderError::Init)
}

/// Device, swap chain and painter. Drops in field order after detaching the target.
struct Stack {
    painter: SheetPainter,
    surface: SheetSurface,
    gpu: Gpu,
}

impl Stack {
    fn new(hwnd: isize, size: Size, palette: Palette) -> windows::core::Result<Self> {
        let gpu = Gpu::new(false)?;
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        let surface = SheetSurface::new(&gpu, hwnd, (size.0, size.1), size.2)?;
        let painter = SheetPainter::new(&gpu, palette)?;
        Ok(Self {
            painter,
            surface,
            gpu,
        })
    }

    /// Resizes when `resized`, then paints and presents `list` when there is one. Returns
    /// whether a frame was presented.
    fn frame(
        &mut self,
        size: Size,
        resized: bool,
        list: Option<&DisplayList>,
    ) -> windows::core::Result<bool> {
        if resized {
            self.surface.resize(&self.gpu, (size.0, size.1), size.2)?;
        }
        let Some(list) = list else {
            return Ok(false);
        };
        self.painter.draw(self.gpu.context(), list, size.2)?;
        self.surface.present()?;
        Ok(true)
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        SheetSurface::release(&self.gpu);
    }
}

/// The thread body: build the stack, report the start, run the loop under a panic guard.
fn sheet_main(
    config: SheetConfig,
    shared: &Shared,
    on_fatal: &(dyn Fn(String) + Send),
    ready: &SyncSender<windows::core::Result<()>>,
) {
    let size = (config.width_px, config.height_px, config.scale);
    let stack = match Stack::new(config.hwnd, size, config.palette) {
        Ok(stack) => stack,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let run = catch_unwind(AssertUnwindSafe(|| run(stack, &config, shared)));
    match run {
        Ok(Ok(())) => {}
        Ok(Err(message)) => on_fatal(message),
        Err(payload) => on_fatal(format!(
            "sheet thread panicked: {}",
            payload_text(&*payload)
        )),
    }
}

/// True when `next` is usable (non-zero pixels, a finite positive scale) and differs from
/// `current`.
fn is_new_size(next: Size, current: Size) -> bool {
    next.0 > 0
        && next.1 > 0
        && next.2.is_finite()
        && next.2 > 0.0
        && (next.0, next.1, next.2.to_bits()) != (current.0, current.1, current.2.to_bits())
}

/// Waits for lists and sizes and paints them. A failing frame rebuilds the whole stack once in
/// the thread's life and is retried; a second failure ends the thread with its message.
fn run(mut stack: Stack, config: &SheetConfig, shared: &Shared) -> Result<(), String> {
    let mut size = (config.width_px, config.height_px, config.scale);
    let mut list: Option<Arc<DisplayList>> = None;
    let mut rebuilt = false;
    while let Some((new_list, new_size)) = shared.wait() {
        let resized = new_size.is_some_and(|next| is_new_size(next, size));
        if let Some(next) = new_size.filter(|_| resized) {
            size = next;
        }
        let fresh = new_list.is_some();
        if fresh {
            list = new_list;
        }
        if !resized && !fresh {
            continue;
        }
        let painted = match stack.frame(size, resized, list.as_deref()) {
            Ok(painted) => painted,
            Err(error) if !rebuilt => {
                rebuilt = true;
                // The old target must go first: a window takes one composition target.
                drop(stack);
                stack = Stack::new(config.hwnd, size, config.palette)
                    .map_err(|e| format!("sheet renderer rebuild failed after {error}: {e}"))?;
                stack
                    .frame(size, false, list.as_deref())
                    .map_err(|e| format!("sheet renderer failed after a rebuild: {e}"))?
            }
            Err(error) => return Err(format!("sheet renderer failed after a rebuild: {error}")),
        };
        if painted {
            shared.paints.fetch_add(1, Ordering::Relaxed);
        }
    }
    Ok(())
}
