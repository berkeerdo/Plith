//! State shared by the window procedure, the `WinEvent` hook and the `Shell` handle. Everything
//! here lives on the UI thread, so plain `Cell` and `RefCell` are enough.

use crate::hit::mask::MaskBuffer;
use crate::window::ShellEvent;
use plith_core::launch::HitMode;
use plith_core::layout::SurfaceLayout;
use plith_core::scene::{ShapeCell, Suppression};
use plith_core::topmost::TopmostFight;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
    pub hit_mode: HitMode,
    pub shapes: Arc<ShapeCell>,
    pub pending: Arc<AtomicBool>,
    pub visual: Cell<isize>,
    pub input: Cell<isize>,
    pub fight: Cell<TopmostFight>,
    pub culprit: RefCell<Option<String>>,
    pub suppression: Cell<Suppression>,
    pub stats: Cell<HitStats>,
    pub hit_errors: Cell<u64>,
    pub registrations: RefCell<Registrations>,
    pub mask: RefCell<Option<MaskBuffer>>,
    on_event: RefCell<Box<dyn FnMut(ShellEvent)>>,
    queue: RefCell<VecDeque<ShellEvent>>,
    draining: Cell<bool>,
}

impl Inner {
    pub(crate) fn new(
        layout: SurfaceLayout,
        hit_mode: HitMode,
        shapes: Arc<ShapeCell>,
        on_event: Box<dyn FnMut(ShellEvent)>,
    ) -> Self {
        Self {
            layout,
            hit_mode,
            shapes,
            pending: Arc::new(AtomicBool::new(false)),
            visual: Cell::new(0),
            input: Cell::new(0),
            fight: Cell::new(TopmostFight::default()),
            culprit: RefCell::new(None),
            suppression: Cell::new(Suppression::default()),
            stats: Cell::new(HitStats::default()),
            hit_errors: Cell::new(0),
            registrations: RefCell::new(Registrations::default()),
            mask: RefCell::new(None),
            on_event: RefCell::new(on_event),
            queue: RefCell::new(VecDeque::new()),
            draining: Cell::new(false),
        }
    }

    /// Delivers an event to the app callback. If the callback itself triggers another event
    /// (a nested message), the event is queued and delivered after the callback returns.
    pub(crate) fn emit(&self, event: ShellEvent) {
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

    /// The window handles that belong to the shell, visual first.
    pub(crate) fn windows(&self) -> Vec<isize> {
        let mut all = vec![self.visual.get()];
        if self.input.get() != 0 {
            all.push(self.input.get());
        }
        all
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
