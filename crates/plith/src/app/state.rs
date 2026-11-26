//! The UI-thread state of the app: one [`App`] reached by the shell handler and the panic hook.

use super::feed::drag::DRAG_TIMER;
use super::feed::{COLLAPSE_TIMER, Feed};
use super::input::drag::ActiveDrag;
use super::saver::Saver;
use super::shelf::{SHELF_TIMER, ShelfHost};
use super::weather_link::WeatherWiring;
use super::wire::raised_line;
use crate::osd::OsdSink;
use crate::update::UpdateLink;
use plith_core::demo::DemoStep;
use plith_core::display::Display;
use plith_core::input::CollapseTimer;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_core::settings::{IdleLook, Settings};
use plith_core::theme::Palette;
use plith_core::theme::registry::{AccentPick, Picks, Registry, ResolvedTheme, ThemeId};
use plith_host::log::LogFile;
use plith_render::{RelayoutHandle, StopHandle};
use plith_shell::ShellEvent;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Instant;

/// Something that stops and joins the render thread. A trait so tests can use a fake.
pub trait Stopper {
    /// Asks the render thread to end and waits for it.
    fn stop_and_join(&self);
}

impl Stopper for StopHandle {
    fn stop_and_join(&self) {
        StopHandle::stop_and_join(self);
    }
}

/// Something that makes the render thread follow a new display. A trait so tests can use a
/// fake.
pub trait Relayouter {
    /// Asks the render thread to paint for `display`.
    fn relayout(&self, display: Display);

    /// Asks the render thread to draw in `palette`.
    fn set_palette(&self, _palette: Palette) {}
}

impl Relayouter for RelayoutHandle {
    fn relayout(&self, display: Display) {
        RelayoutHandle::relayout(self, display);
    }

    fn set_palette(&self, palette: Palette) {
        RelayoutHandle::set_palette(self, palette);
    }
}

/// Everything the UI thread owns. It lives in an `Rc<RefCell<App>>` shared by the shell handler
/// and the panic hook; both run on the UI thread only.
///
/// Re-entrancy: the shell may deliver an event while an earlier `emit` is draining. Never hold
/// the `borrow_mut` across a call that can pump messages or re-enter `emit`. `Closing` joins the
/// render thread, which never sends to the UI thread, so that path does not re-enter.
pub struct App {
    /// The scene the render thread reads.
    pub mailbox: Arc<SceneMailbox>,
    /// The last frame the render thread drew: its shape and page layers.
    pub shapes: Arc<ShapeCell>,
    /// The run log.
    pub log: Arc<LogFile>,
    /// Set on `Closing`; the render sinks read it from another thread, so it stays an `Arc`.
    pub closing: Arc<AtomicBool>,
    stop: Option<Box<dyn Stopper>>,
    /// Sends display changes to the render thread, once it has been spawned.
    relayout: Option<Box<dyn Relayouter>>,
    pub(super) feed: Option<Feed>,
    /// Monotonic seconds, for the collapse delay.
    pub(super) now: Box<dyn Fn() -> f64>,
    /// Armed while the pointer is outside the open island.
    pub(super) collapse: CollapseTimer,
    /// Scene steps from the demo thread, applied on the next wake.
    pub(super) demo: Option<Receiver<DemoStep>>,
    /// The weather wiring of the run, to start the weather when the user turns it on.
    weather: Option<WeatherWiring>,
    /// The OSD side of a normal run, switched by the settings.
    osd: Option<OsdControl>,
    /// True in a normal run: only it reads and writes the play log.
    normal_run: bool,
    /// The ruler drag under way, from the press to the release.
    pub(super) drag: Option<ActiveDrag>,
    /// The shelf: the file drag hover, the wagon pressed and the drag out.
    pub(super) shelf: ShelfHost,
    /// The settings of the run; the app is their one owner and applies every change.
    settings: Settings,
    /// The look the settings file holds while `--idle-look` overrides it for this run: saves
    /// write it until the user picks a look.
    file_look: Option<IdleLook>,
    /// The theme the settings file holds while `--theme` overrides it for this run; same rule.
    file_theme: Option<ThemeId>,
    /// The theme packs of the run, built at start.
    registry: Registry,
    /// Writes the settings file; a normal run only.
    saver: Option<Saver>,
    /// The settings window.
    sheet: SheetHost,
    /// The tray icon's menu captions and its way to quit.
    tray: TrayHost,
    /// The update service of a normal run.
    update: Option<UpdateLink>,
}

impl App {
    /// A new app with no render thread attached yet; `shapes` is the cell the shell and the
    /// render thread share.
    #[must_use]
    pub fn new(
        mailbox: Arc<SceneMailbox>,
        shapes: Arc<ShapeCell>,
        log: Arc<LogFile>,
        closing: Arc<AtomicBool>,
    ) -> Self {
        let origin = Instant::now();
        Self {
            mailbox,
            shapes,
            log,
            closing,
            stop: None,
            relayout: None,
            feed: None,
            now: Box::new(move || origin.elapsed().as_secs_f64()),
            collapse: CollapseTimer::default(),
            demo: None,
            drag: None,
            shelf: ShelfHost::default(),
            weather: None,
            osd: None,
            normal_run: false,
            settings: Settings::default(),
            file_look: None,
            file_theme: None,
            registry: Registry::builtin(),
            saver: None,
            sheet: SheetHost::default(),
            tray: TrayHost::default(),
            update: None,
        }
    }

    /// The palettes of the run's theme under the user's picks. The live Windows accent is not
    /// wired yet, so a `Windows` pick falls back to the pack's own flow.
    #[must_use]
    pub fn resolved(&self) -> ResolvedTheme {
        self.resolve(&self.settings)
    }

    /// `resolved` for the given `settings`.
    pub(super) fn resolve(&self, settings: &Settings) -> ResolvedTheme {
        let id = &settings.theme;
        let accent: Option<&AccentPick> = settings.accents.get(id);
        let picks = Picks {
            accent,
            floor: settings.floors.get(id).copied(),
            windows_accent: None,
        };
        self.registry.resolve(id, picks)
    }

    /// Replaces the monotonic clock (seconds) the collapse delay reads.
    #[cfg(test)]
    #[must_use]
    pub fn with_clock(self, now: Box<dyn Fn() -> f64>) -> Self {
        Self { now, ..self }
    }

    /// Starts feeding the pages from the services; until then a wake or a timer does nothing.
    pub fn attach_feed(&mut self, feed: Feed) {
        self.feed = Some(feed);
    }

    /// Hands the feed's volume and brightness changes to `sink`; without a feed it is dropped.
    pub fn set_osd(&mut self, sink: OsdSink) {
        if let Some(feed) = &mut self.feed {
            feed.set_osd(sink);
        }
    }

    /// Takes the demo's scene steps from `steps`; each wake applies the queued ones.
    pub fn attach_demo(&mut self, steps: Receiver<DemoStep>) {
        self.demo = Some(steps);
    }

    /// Stops the services, once the render thread has been joined.
    pub fn stop_feed(&mut self) {
        if let Some(feed) = &mut self.feed {
            feed.stop();
        }
    }

    /// Lets the feed react to a scene change.
    pub(super) fn scene_changed(&mut self) {
        if let Some(feed) = &mut self.feed {
            feed.on_scene(&self.mailbox);
        }
    }

    /// Records how to stop the render thread, once it has been spawned.
    pub fn attach_render(&mut self, stop: impl Stopper + 'static) {
        self.stop = Some(Box::new(stop));
    }

    /// Records how to send display changes to the render thread.
    pub fn attach_relayout(&mut self, relayout: impl Relayouter + 'static) {
        self.relayout = Some(Box::new(relayout));
    }

    /// The window already moved to `display` (the shell did it); log it and let the render
    /// thread follow.
    fn on_display_changed(&self, display: Display) {
        self.log.line(&display.log_line());
        if let Some(relayout) = &self.relayout {
            relayout.relayout(display);
        }
    }

    /// Stops and joins the render thread under the close backstop, if one is attached.
    pub fn close_render(&self) {
        if let Some(stop) = &self.stop {
            super::backstop::close_render(&self.log, stop.as_ref());
        }
    }

    /// Handles one shell event. On `Closing` (`WM_CLOSE`, before the window is destroyed) it sets
    /// `closing`, then stops and joins the render thread, which presents to the window about to
    /// be destroyed.
    pub fn on_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::Closing => {
                self.closing.store(true, Ordering::Release);
                self.close_render();
            }
            ShellEvent::Click { x, y } => self.on_click(x, y),
            ShellEvent::Press { x, y } => self.on_press(x, y),
            ShellEvent::Drag { x, y } => self.on_drag(x, y),
            ShellEvent::Release { x, y } => self.on_release(x, y),
            ShellEvent::Timer(DRAG_TIMER) => {
                let now = (self.now)();
                if let Some(feed) = &mut self.feed {
                    feed.on_drag_timer(now, &self.mailbox);
                }
            }
            ShellEvent::Enter => self.on_enter(),
            ShellEvent::Move { x, y } => self.on_move(x, y),
            ShellEvent::Leave => self.on_leave(),
            ShellEvent::Timer(COLLAPSE_TIMER) => self.on_collapse_tick(),
            ShellEvent::Timer(SHELF_TIMER) => self.on_shelf_tick(),
            ShellEvent::DragEnter {
                x,
                y,
                formats,
                effect,
            } => self.on_drag_enter(formats, effect, (x, y)),
            ShellEvent::DragOver { .. } => self.on_drag_over(),
            ShellEvent::DragLeave => self.on_drag_leave(),
            ShellEvent::DragRefused { formats } => self.on_drag_refused(formats),
            ShellEvent::Drop { paths } => self.on_drop(&paths),
            ShellEvent::Timer(CARET_TIMER) => self.on_caret_tick(),
            ShellEvent::Tray(item) => self.on_tray(item),
            ShellEvent::TrayNotice => self.on_tray_notice(),
            ShellEvent::Suppression(suppression) => {
                let _ = self.mailbox.update(|scene| Scene {
                    suppression,
                    ..scene
                });
                self.scene_changed();
                self.log.line(&format!(
                    "suppression: display_off={} locked={} fullscreen={}",
                    suppression.display_off, suppression.locked, suppression.fullscreen
                ));
            }
            ShellEvent::Wake => {
                self.apply_demo_steps();
                self.poll_osd();
                if let Some(feed) = &mut self.feed {
                    feed.on_wake(&self.mailbox);
                }
                self.drain_sheet();
                self.drain_shelf_load();
                self.drain_update();
            }
            ShellEvent::Timer(id) => {
                if let Some(feed) = &mut self.feed {
                    feed.on_timer(id, &self.mailbox);
                }
                // The minute tick rebuilds the pill; the window's previews follow it.
                self.refresh_sheet();
            }
            ShellEvent::DisplayChanged(display) => self.on_display_changed(display),
            ShellEvent::Raised {
                foreground,
                above,
                immediate,
            } => self
                .log
                .line(&raised_line(&foreground, above.as_deref(), immediate)),
        }
    }
}

mod live;
mod sheet;
mod tray;
mod update;

pub use live::OsdControl;
use sheet::{CARET_TIMER, SheetHost};
use tray::TrayHost;

#[cfg(test)]
mod tests;
