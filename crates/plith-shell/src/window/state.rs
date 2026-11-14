//! State shared by the window procedure, the `WinEvent` hook and the `Shell` handle. Everything
//! here lives on the UI thread, so plain `Cell` and `RefCell` are enough.

use crate::handles::hwnd_of;
use crate::poster::Coalesce;
use crate::window::ShellEvent;
use plith_core::layout::SurfaceLayout;
use plith_core::scene::{ShapeCell, Suppression};
use plith_core::topmost::TopmostFight;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, PostQuitMessage, WM_CLOSE};

/// Running cost of applying a shape to the hit candidate, in milliseconds.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HitStats {
    pub count: u64,
    pub total_ms: f64,
    pub max_ms: f64,
}

/// Handles that must be released on `WM_DESTROY`.
#[derive(Debug, Default)]
pub(crate) struct Registrations {
    pub power: Option<isize>,
    pub win_event_hook: Option<isize>,
    pub session: bool,
    pub appbar: bool,
}

pub(crate) struct Inner {
    pub layout: SurfaceLayout,
    pub shapes: Arc<ShapeCell>,
    pub pending: Arc<Coalesce>,
    /// Coalescer of the generic UI wake; cleared when `WM_APP_WAKE` is handled.
    pub wake: Arc<Coalesce>,
    pub visual: Cell<isize>,
    pub fight: Cell<TopmostFight>,
    pub culprit: RefCell<Option<String>>,
    pub suppression: Cell<Suppression>,
    pub stats: Cell<HitStats>,
    pub hit_errors: Cell<u64>,
    pub registrations: RefCell<Registrations>,
    /// Origin of the monotonic clock the topmost debounce reads.
    pub origin: Instant,
    on_event: RefCell<Box<dyn FnMut(ShellEvent)>>,
    on_panic: Box<dyn Fn(String)>,
    panicked: Cell<bool>,
    queue: RefCell<VecDeque<ShellEvent>>,
    draining: Cell<bool>,
}

impl Inner {
    pub(crate) fn new(
        layout: SurfaceLayout,
        shapes: Arc<ShapeCell>,
        on_event: Box<dyn FnMut(ShellEvent)>,
        on_panic: Box<dyn Fn(String)>,
    ) -> Self {
        Self {
            layout,
            shapes,
            pending: Arc::new(Coalesce::default()),
            wake: Arc::new(Coalesce::default()),
            visual: Cell::new(0),
            fight: Cell::new(TopmostFight::default()),
            culprit: RefCell::new(None),
            suppression: Cell::new(Suppression::default()),
            stats: Cell::new(HitStats::default()),
            hit_errors: Cell::new(0),
            registrations: RefCell::new(Registrations::default()),
            origin: Instant::now(),
            on_event: RefCell::new(on_event),
            on_panic,
            panicked: Cell::new(false),
            queue: RefCell::new(VecDeque::new()),
            draining: Cell::new(false),
        }
    }

    /// Delivers an event to the app callback. If the callback itself triggers another event
    /// (a nested message), the event is queued and delivered after the callback returns.
    /// After a panic the app handler is never entered again.
    pub(crate) fn emit(&self, event: ShellEvent) {
        if self.panicked.get() {
            return;
        }
        self.queue.borrow_mut().push_back(event);
        if self.draining.replace(true) {
            return;
        }
        loop {
            let next = self.queue.borrow_mut().pop_front();
            let Some(event) = next else { break };
            (self.on_event.borrow_mut())(event);
        }
        self.draining.set(false);
    }

    /// Records a caught panic: reports it once through `on_panic`, mutes the app handler and
    /// asks the window to close. Later panics are already covered by the first report.
    pub(crate) fn fail(&self, message: String) {
        if self.panicked.replace(true) {
            return;
        }
        (self.on_panic)(message);
        let visual = self.visual.get();
        if visual == 0 {
            // No window exists to close, so end the message loop directly.
            // SAFETY: posts WM_QUIT to this thread's queue.
            unsafe { PostQuitMessage(1) };
        } else {
            // SAFETY: PostMessageW accepts any handle value; a stale one only makes it fail.
            let _ = unsafe { PostMessageW(Some(hwnd_of(visual)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    /// The window handles that belong to the shell.
    pub(crate) fn windows(&self) -> Vec<isize> {
        vec![self.visual.get()]
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Inner>>> = const { RefCell::new(None) };
}

pub(crate) fn set_current(inner: Option<Rc<Inner>>) {
    CURRENT.with(|c| *c.borrow_mut() = inner);
}

/// The shell state of this thread, if a shell exists.
pub(crate) fn current() -> Option<Rc<Inner>> {
    CURRENT.with(|c| c.borrow().clone())
}
