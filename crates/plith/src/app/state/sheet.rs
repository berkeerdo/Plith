//! The settings window as the app drives it: opening (or raising) it, draining its events on a
//! wake, applying their steps, keeping its list current and closing it.
//!
//! Re-entrancy: the window's callback only queues the event and posts a wake; it never borrows
//! the app. Opening happens inside the app's own event handling, which is safe because
//! `SheetWindow::open` never calls back, and the island's shell queues nested events.

use super::{App, OsdControl};
use crate::app::feed::Feed;
use crate::app::idle_wiring::language_of;
use crate::app::sheet_side::{Opening, SheetSide, Step, blank};
use crate::settings_file;
use plith_core::draw::DisplayList;
use plith_core::lang::Lang;
use plith_core::lang::sheet::{SheetText, sheet_text};
use plith_core::pages::idle_pill::{IdleFacts, idle_pill};
use plith_core::settings::change::SettingChange;
use plith_core::settings::{IdleLook, Settings};
use plith_core::sheet::{Section, SheetFacts, SheetView, commit_field};
use plith_core::theme::Theme;
use plith_host::autostart::Autostart;
use plith_host::log::LogFile;
use plith_shell::FIRST_APP_TIMER;
use plith_shell::sheet_window::{SheetEvent, set_clipboard};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

/// The island timer that blinks the city field's caret while the window is open and the field
/// has focus; it runs at no other time.
pub const CARET_TIMER: usize = FIRST_APP_TIMER + 3;
/// The caret blink period.
const CARET_MS: u32 = 530;

/// Wakes the UI thread; callable from any thread.
type Wake = Arc<dyn Fn() + Send + Sync>;

/// What the six previews were built from, and the previews.
type Previews = (IdleFacts, [Arc<DisplayList>; 6]);

/// The facts a list was last built with, besides the previews: the settings, the language, a
/// failed save and the start with Windows switch.
type Shown = (Settings, Lang, bool, bool);

/// The settings window's place in the app, open or not.
pub struct SheetHost {
    /// Window events waiting for the next wake.
    queue: Rc<RefCell<VecDeque<SheetEvent>>>,
    wake: Wake,
    /// The monitor the island is on (`HMONITOR` value).
    monitor: isize,
    /// The settings file, shown in the About section.
    settings_path: String,
    /// Its folder, opened by the About button.
    folder: Option<PathBuf>,
    /// The open window.
    side: Option<SheetSide>,
    /// The section shown last in this run; the next opening starts there.
    section: Section,
    previews: Option<Previews>,
    shown: Option<Shown>,
    caret_armed: bool,
    /// Whether the app attached the window; a tray command before that opens nothing.
    attached: bool,
    /// The start with Windows switch; `None` in tests and runs that never touch the registry.
    autostart: Option<Box<dyn Autostart>>,
    /// The switch as last read or written, shown by the Genel row.
    autostart_on: bool,
    /// How often `refresh_sheet` ran, for the tests.
    #[cfg(test)]
    pub(in crate::app) refreshes: u32,
}

impl Default for SheetHost {
    fn default() -> Self {
        Self {
            queue: Rc::default(),
            wake: Arc::new(|| {}),
            monitor: 0,
            settings_path: String::new(),
            folder: None,
            side: None,
            section: Section::default(),
            previews: None,
            shown: None,
            caret_armed: false,
            attached: false,
            autostart: None,
            autostart_on: false,
            #[cfg(test)]
            refreshes: 0,
        }
    }
}

impl App {
    /// Lets the quick page open the settings window on `monitor`; `wake` posts a wake to the
    /// UI thread, where the window's events are drained.
    pub fn attach_sheet(&mut self, monitor: isize, wake: Wake) {
        let path = settings_file::default_path().ok();
        self.sheet.settings_path = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.sheet.folder = path.and_then(|p| p.parent().map(PathBuf::from));
        self.sheet.monitor = monitor;
        self.sheet.wake = wake;
        self.sheet.attached = true;
    }

    /// Gives the Genel row its start with Windows switch (a normal run only).
    pub fn attach_autostart(&mut self, autostart: Box<dyn Autostart>) {
        self.sheet.autostart = Some(autostart);
    }

    /// Reads the switch again, so the row shows what the registry holds now.
    fn read_autostart(&mut self) {
        if let Some(autostart) = &self.sheet.autostart {
            self.sheet.autostart_on = autostart.is_on();
        }
    }

    /// Turns start with Windows on or off. A failed write is logged and the row shows what the
    /// registry holds afterwards, so it springs back instead of lying.
    pub(in crate::app) fn set_autostart(&mut self, on: bool) {
        let Some(autostart) = self.sheet.autostart.as_mut() else {
            return;
        };
        match autostart.set(on) {
            Ok(()) => self
                .log
                .line(&format!("autostart: {}", if on { "on" } else { "off" })),
            Err(error) => self.log.line(&format!("autostart: cannot switch: {error}")),
        }
        self.read_autostart();
    }

    /// Opens the settings window on the section shown last, or raises the open one.
    pub(in crate::app) fn open_sheet(&mut self) {
        self.read_autostart();
        if !self.sheet.attached {
            self.log.line("settings window: not attached");
            return;
        }
        if let Some(side) = &self.sheet.side {
            side.raise();
            self.refresh_sheet();
            return;
        }
        self.update_previews();
        let facts = self.sheet_facts();
        let view = SheetView {
            section: self.sheet.section,
            ..SheetView::default()
        };
        match SheetSide::open(self.opening(facts.lang, facts.settings.theme), view) {
            Ok(mut side) => {
                if !side.has_icon() {
                    self.log.line("settings window: icon decode failed");
                }
                side.publish(&facts);
                self.sheet.side = Some(side);
                self.sheet.shown = Some(shown(&facts));
                self.sheet_clock(true);
                self.log.line("settings window: open");
            }
            Err(line) => self.log.line(&line),
        }
    }

    fn opening(&self, lang: Lang, theme: Theme) -> Opening {
        let queue = Rc::clone(&self.sheet.queue);
        let wake = Arc::clone(&self.sheet.wake);
        let on_event = Box::new(move |event| {
            queue.borrow_mut().push_back(event);
            wake();
        });
        let (log, wake) = (Arc::clone(&self.log), Arc::clone(&self.sheet.wake));
        let on_panic = Box::new(move |message: String| {
            log.line(&format!("settings window: panic: {message}"));
            wake();
        });
        let (log, wake) = (Arc::clone(&self.log), Arc::clone(&self.sheet.wake));
        let on_fatal = Box::new(move |message: String| {
            log.line(&format!("settings window: render failed: {message}"));
            wake();
        });
        Opening {
            monitor: self.sheet.monitor,
            title: sheet_text(SheetText::WindowTitle, lang).to_owned(),
            theme,
            on_event,
            on_panic,
            on_fatal,
        }
    }

    /// On a wake: applies the window's queued events in order, then brings its list up to date.
    pub(super) fn drain_sheet(&mut self) {
        loop {
            if self.sheet.side.as_ref().is_some_and(SheetSide::broken) {
                self.log.line("settings window: closed after a failure");
                self.close_sheet();
            }
            let next = self.sheet.queue.borrow_mut().pop_front();
            let Some(event) = next else { break };
            let facts = self.sheet_facts();
            // Events of a window closed earlier in this drain are dropped.
            let Some(side) = self.sheet.side.as_mut() else {
                continue;
            };
            for step in side.route(event, &facts) {
                self.sheet_step(step);
            }
        }
        self.refresh_sheet();
    }

    fn sheet_step(&mut self, step: Step) {
        match step {
            // The side marked itself for a rebuild already.
            Step::Redraw => {}
            Step::Change(change) => self.change(change),
            Step::Close => self.close_sheet(),
            Step::Copy(text) => set_clipboard(&text),
            Step::OpenFolder => self.open_folder(),
            Step::Autostart(on) => self.set_autostart(on),
            Step::Resize(width_px, height_px, scale) => {
                if let Some(side) = &self.sheet.side {
                    side.resize(width_px, height_px, scale);
                }
            }
        }
    }

    /// Republishes the open window's list when its view or facts changed (a setting, the
    /// language, a failed save, the previews when the pill rebuilt), keeps its title in the
    /// current language and the caret timer in step with the field focus.
    pub(super) fn refresh_sheet(&mut self) {
        #[cfg(test)]
        {
            self.sheet.refreshes += 1;
        }
        if self.sheet.side.is_none() {
            return;
        }
        let previews_moved = self.update_previews();
        let facts = self.sheet_facts();
        let now = shown(&facts);
        if let Some(side) = self.sheet.side.as_mut() {
            if previews_moved || self.sheet.shown.as_ref() != Some(&now) {
                side.touch();
            }
            side.set_title(sheet_text(SheetText::WindowTitle, facts.lang));
            side.set_theme(facts.settings.theme);
            side.publish(&facts);
        }
        self.sheet.shown = Some(now);
        self.sync_caret();
    }

    /// The caret timer ticked: blink while the field has focus, else stop the timer.
    pub(super) fn on_caret_tick(&mut self) {
        if let Some(side) = self.sheet.side.as_mut()
            && side.caret_wanted()
        {
            side.blink();
        }
        self.refresh_sheet();
    }

    /// Runs the feed's minute clock while the window is open, for the previews.
    pub(super) fn sheet_clock(&mut self, open: bool) {
        if let Some(feed) = &mut self.feed {
            feed.set_sheet_open(open, &self.mailbox);
        }
    }

    fn sync_caret(&mut self) {
        let wanted = self
            .sheet
            .side
            .as_ref()
            .is_some_and(SheetSide::caret_wanted);
        if wanted == self.sheet.caret_armed {
            return;
        }
        let Some(feed) = &self.feed else { return };
        if wanted {
            feed.timers().set(CARET_TIMER, CARET_MS);
        } else {
            feed.timers().kill(CARET_TIMER);
        }
        self.sheet.caret_armed = wanted;
    }

    /// Closes the window, if open: a city still being typed is committed and applied first.
    /// Remembers the section for the next opening.
    pub fn close_sheet(&mut self) {
        self.close_sheet_with(true);
    }

    /// `close_sheet`; with `apply` false (at exit) the committed city is only saved.
    pub(super) fn close_sheet_with(&mut self, apply: bool) {
        let Some(mut side) = self.sheet.side.take() else {
            return;
        };
        let city = commit_field(&mut side.view);
        let view = side.close();
        self.sheet.section = view.section;
        self.sheet.shown = None;
        self.sheet.previews = None;
        self.sheet_clock(false);
        self.sync_caret();
        self.log.line("settings window: closed");
        if let Some(city) = city {
            if apply {
                self.change(SettingChange::City(city));
            } else {
                self.save_change(SettingChange::City(city));
            }
        }
    }

    /// Rebuilds the six previews when the pill's facts moved; true when they did.
    fn update_previews(&mut self) -> bool {
        let Some(facts) = self.feed.as_ref().and_then(Feed::idle_facts) else {
            return false;
        };
        if self
            .sheet
            .previews
            .as_ref()
            .is_some_and(|(f, _)| f == facts)
        {
            return false;
        }
        let lists = static_previews(facts);
        self.sheet.previews = Some((facts.clone(), lists));
        true
    }

    /// What the window shows besides its view state.
    pub(super) fn sheet_facts(&self) -> SheetFacts {
        let lang = self
            .feed
            .as_ref()
            .map_or_else(|| language_of(&self.settings), Feed::lang);
        let previews = self.sheet.previews.as_ref().map_or_else(
            || std::array::from_fn(|_| Arc::new(blank())),
            |(_, lists)| lists.clone(),
        );
        SheetFacts {
            settings: self.settings.clone(),
            lang,
            previews,
            version: env!("CARGO_PKG_VERSION"),
            build_date: env!("PLITH_BUILD_DATE"),
            flyout_locked: self.osd.as_ref().is_some_and(OsdControl::flyout_locked),
            settings_path: self.sheet.settings_path.clone(),
            save_failed: self.save_failed(),
            autostart: self.sheet.autostart_on,
        }
    }

    /// Opens the settings folder in Explorer, on a short thread that waits for the launcher.
    fn open_folder(&self) {
        let Some(folder) = self.sheet.folder.clone() else {
            return;
        };
        let log: Arc<LogFile> = Arc::clone(&self.log);
        let spawned = std::thread::Builder::new()
            .name(String::from("plith-open-folder"))
            .spawn(move || {
                if let Err(error) = Command::new("explorer.exe").arg(&folder).status() {
                    log.line(&format!("settings window: cannot open the folder: {error}"));
                }
            });
        if let Err(error) = spawned {
            self.log
                .line(&format!("settings window: cannot open the folder: {error}"));
        }
    }
}

/// The six previews of `facts`, with no running title scroll: a scrolling title is a live node,
/// which a preview skips, so the title would vanish for the seconds the scroll runs.
fn static_previews(facts: &IdleFacts) -> [Arc<DisplayList>; 6] {
    let still = IdleFacts {
        scroll_start: None,
        ..facts.clone()
    };
    IdleLook::ALL.map(|look| Arc::new(idle_pill(look, &still)))
}

fn shown(facts: &SheetFacts) -> Shown {
    (
        facts.settings.clone(),
        facts.lang,
        facts.save_failed,
        facts.autostart,
    )
}

#[cfg(test)]
mod tests;
