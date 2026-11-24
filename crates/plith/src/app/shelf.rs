//! The shelf on the app side: a file drag over the island opens the shelf after a dwell (ADR
//! 0018), a drop puts the files on it, and a wagon pressed and pulled past the slop is dragged
//! out to whatever window the pointer reaches. One `drag` line is logged per drag.

use super::feed::Feed;
use super::state::App;
use plith_core::input::page_on_screen;
use plith_core::page::PageLayers;
use plith_core::pages::{Action, PageId, hit};
use plith_core::shelf::hover::{Cmd, DragHover};
use plith_core::shelf::line::{DragIn, DragOut, InEnd};
use plith_core::shelf::{Shelf, ShelfItem, newest};
use plith_shell::{DragOutReport, FIRST_APP_TIMER};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// The one-shot that ends the dwell or the grace of a file drag.
pub const SHELF_TIMER: usize = FIRST_APP_TIMER + 10;

/// Runs a drag out and reports how it went. A box so tests can fake it.
pub type DragOutRunner = Box<dyn Fn(&[PathBuf]) -> DragOutReport>;

/// What the log line of a drag into the island collects.
struct InNote {
    formats: String,
    open_ms: Option<u32>,
    effect: &'static str,
    point: (i32, i32),
    enters: u32,
}

/// The shelf state of the app.
pub struct ShelfHost {
    hover: DragHover,
    note: Option<InNote>,
    /// Milliseconds to the open of the running hover session, kept across a leave and a return
    /// inside the grace.
    opened_ms: Option<u32>,
    /// How many times the running hover session entered the island.
    enters: u32,
    /// The saved shelf being read off the UI thread, until it arrives.
    load: Option<Receiver<Shelf>>,
    /// The path of the wagon pressed, until the press ends or becomes a drag.
    pressed: Option<PathBuf>,
    out: DragOutRunner,
}

impl Default for ShelfHost {
    fn default() -> Self {
        Self {
            hover: DragHover::default(),
            note: None,
            opened_ms: None,
            enters: 0,
            load: None,
            pressed: None,
            out: Box::new(plith_shell::start_drag_out),
        }
    }
}

impl ShelfHost {
    /// A host whose drags out go through `out` instead of OLE.
    #[cfg(test)]
    pub fn with_runner(out: DragOutRunner) -> Self {
        Self {
            out,
            ..Self::default()
        }
    }
}

/// Seconds as a timer period in milliseconds, at least one.
fn period_ms(seconds: f64) -> u32 {
    // Allowed: the periods are a few hundred milliseconds.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ms = (seconds * 1000.0).ceil().max(1.0) as u32;
    ms
}

impl App {
    /// A file drag entered the island.
    pub(super) fn on_drag_enter(&mut self, formats: String, effect: &'static str, at: (f64, f64)) {
        if !self.shelf.hover.active() {
            self.shelf.opened_ms = None;
            self.shelf.enters = 0;
        }
        self.shelf.enters += 1;
        // Allowed: island DIP points are small values; the log wants whole numbers.
        #[allow(clippy::cast_possible_truncation)]
        let point = (at.0.round() as i32, at.1.round() as i32);
        self.shelf.note = Some(InNote {
            formats,
            open_ms: self.shelf.opened_ms,
            effect,
            point,
            enters: self.shelf.enters,
        });
        self.on_drag_over();
    }

    /// The file drag is over the island: the dwell runs, or the grace ends.
    pub(super) fn on_drag_over(&mut self) {
        let cmd = self.shelf.hover.over((self.now)());
        if self.shelf.hover.hovering() {
            self.cancel_collapse();
        }
        self.apply_hover(cmd);
    }

    /// The file drag left the island without a drop.
    pub(super) fn on_drag_leave(&mut self) {
        let cmd = self.shelf.hover.leave((self.now)());
        self.log_in(InEnd::Left, 0);
        self.apply_hover(cmd);
    }

    /// A drag with no file list entered: it got no effect, nothing opens.
    pub(super) fn on_drag_refused(&mut self, formats: String) {
        self.shelf.note = Some(InNote {
            formats,
            open_ms: None,
            effect: "none",
            point: (0, 0),
            enters: 1,
        });
        self.log_in(InEnd::Refused, 0);
    }

    /// Files were dropped: they go on the shelf, which opens when the dwell had not run out.
    pub(super) fn on_drop(&mut self, paths: &[PathBuf]) {
        let cmd = self.shelf.hover.dropped((self.now)());
        // Cut to the eight that can stay before any per-path work: a thousand files cost
        // eight probes, and the source is blocked in its drop call until this returns.
        let items: Vec<ShelfItem> = newest(paths)
            .into_iter()
            .map(|path| ShelfItem {
                dir: std::fs::metadata(&path).is_ok_and(|meta| meta.is_dir()),
                path,
            })
            .collect();
        if let Cmd::Open { ms } = cmd {
            self.note_open(ms);
            self.cancel_collapse();
            self.open_shelf();
        }
        self.kill_shelf_timer();
        if let Some(feed) = self.feed.as_mut() {
            feed.set_shelf_hot(false, &self.mailbox);
            feed.shelf_add(&items, &self.mailbox);
        }
        self.log_in(InEnd::Dropped, items.len());
    }

    /// The dwell or the grace timer ticked.
    pub(super) fn on_shelf_tick(&mut self) {
        self.kill_shelf_timer();
        let cmd = self.shelf.hover.tick((self.now)());
        self.apply_hover(cmd);
    }

    fn apply_hover(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Wait(seconds) => {
                if let Some(feed) = &self.feed {
                    feed.timers().set(SHELF_TIMER, period_ms(seconds));
                }
            }
            Cmd::Open { ms } => {
                self.kill_shelf_timer();
                self.note_open(ms);
                self.cancel_collapse();
                self.open_shelf();
            }
            Cmd::Collapse => self.on_leave(),
            Cmd::Nothing => {}
        }
        let hot = self.shelf.hover.hot();
        if let Some(feed) = self.feed.as_mut() {
            feed.set_shelf_hot(hot, &self.mailbox);
        }
    }

    fn kill_shelf_timer(&self) {
        if let Some(feed) = &self.feed {
            feed.timers().kill(SHELF_TIMER);
        }
    }

    fn note_open(&mut self, ms: u32) {
        self.shelf.opened_ms.get_or_insert(ms);
        if let Some(note) = self.shelf.note.as_mut() {
            note.open_ms.get_or_insert(ms);
        }
    }

    /// Takes the saved shelf once the load thread delivered it: it goes behind whatever was
    /// dropped meanwhile, and the file is written again with both.
    pub(super) fn drain_shelf_load(&mut self) {
        let Some(load) = self.shelf.load.as_ref() else {
            return;
        };
        let Ok(loaded) = load.try_recv() else {
            return;
        };
        self.shelf.load = None;
        if let Some(feed) = self.feed.as_mut() {
            feed.shelf_loaded(&loaded, &self.mailbox);
        }
    }

    /// Hands the app the channel the saved shelf comes through.
    pub fn attach_shelf_load(&mut self, load: Receiver<Shelf>) {
        self.shelf.load = Some(load);
    }

    /// Shows the shelf page, opening the island when it is closed. A hidden island stays so.
    fn open_shelf(&mut self) {
        let scene = self.mailbox.latest().scene;
        if scene.suppression.any() {
            return;
        }
        if !scene.expanded {
            self.expand_on(Some(PageId::Shelf));
        } else if scene.page != PageId::Shelf.index() {
            self.turn_to(PageId::Shelf);
        }
    }

    fn log_in(&mut self, end: InEnd, files: usize) {
        let Some(note) = self.shelf.note.take() else {
            return;
        };
        let line = DragIn {
            formats: note.formats,
            files,
            end,
            open_ms: note.open_ms,
            effect: note.effect,
            point: note.point,
            enters: note.enters,
        }
        .line();
        self.log.line(&line);
    }

    /// The wagon under a press at `(x, y)` island DIP, as the path it stands for.
    fn wagon_under(&self, x: f64, y: f64) -> Option<PathBuf> {
        if !self.mailbox.latest().scene.expanded {
            return None;
        }
        let layers = self
            .shapes
            .latest()
            .map_or_else(PageLayers::empty, |frame| frame.pages);
        let layer = page_on_screen(&layers)?;
        if layer.page != PageId::Shelf.index() {
            return None;
        }
        let feed: &Feed = self.feed.as_ref()?;
        let targets = feed.targets().get(layer.page)?;
        match hit(targets, x - layer.offset, y)? {
            Action::ShelfWagon(index) => feed.shelf_path(index),
            _ => None,
        }
    }

    /// A counted press: remembers the wagon under it.
    pub(super) fn note_wagon_press(&mut self, x: f64, y: f64) {
        self.shelf.pressed = self.wagon_under(x, y);
    }

    /// The press ended as a click or a release: no wagon is pending any more.
    pub(super) fn forget_wagon_press(&mut self) {
        self.shelf.pressed = None;
    }

    /// The pressed pointer passed the slop: when it came from a wagon, the drag out starts and
    /// runs until the button goes up. Returns `true` when it took the drag.
    pub(super) fn take_wagon_drag(&mut self) -> bool {
        let Some(path) = self.shelf.pressed.take() else {
            return false;
        };
        self.shelf.hover.set_own(true);
        let report = (self.shelf.out)(&[path]);
        self.shelf.hover.set_own(false);
        self.log.line(
            &DragOut {
                items: report.offered,
                missing: report.missing,
                effect: report.effect,
                ms: report.ms,
            }
            .line(),
        );
        if !report.inside {
            // The drag swallowed the leave: the pointer is out and nothing else will say so.
            self.on_leave();
        }
        true
    }
}

#[cfg(test)]
mod tests;
