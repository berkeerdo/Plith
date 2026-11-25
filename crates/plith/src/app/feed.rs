//! The feed on the UI thread: service readings become page content, and the clock timer keeps
//! the tab bar current while the island is open. Collapsed, no timer runs (G2). The elapsed
//! time and the seek are live nodes, so no timer rebuilds the media page.

pub mod drag;
mod idle;
mod osd;
mod settings;
mod shelf;

use super::idle_facts::Idle;
use super::weather_link::WeatherLink;
use crate::pages::{Dirty, Inputs, Pages};
use crate::services::{Facts, Services};
use plith_core::input::{MixerShape, pick_within, route_allowed};
use plith_core::lang::Lang;
use plith_core::pages::audio::PickMode;
use plith_core::pages::idle_pill::IdleFacts;
use plith_core::pages::shelf::ShelfView;
use plith_core::pages::tabbar::clock_text;
use plith_core::pages::welcome::{WelcomeFacts, probe_hour};
use plith_core::pages::{HitTarget, PAGE_COUNT, PageId};
use plith_core::scene::{Scene, SceneMailbox, Suppression};
use plith_core::settings::IdleLook;
use plith_host::log::Stamp;
use plith_shell::{FIRST_APP_TIMER, TimerHandle};

/// Sets and kills UI timers. A trait so tests can record instead of calling Windows.
pub trait Timers {
    /// Starts or restarts timer `id` with a period of `ms`.
    fn set(&self, id: usize, ms: u32);
    /// Stops timer `id`.
    fn kill(&self, id: usize);
}

impl Timers for TimerHandle {
    fn set(&self, id: usize, ms: u32) {
        TimerHandle::set(self, id, ms);
    }

    fn kill(&self, id: usize) {
        TimerHandle::kill(self, id);
    }
}

/// A one-shot to the next minute boundary for the tab bar clock while open.
pub const CLOCK_TIMER: usize = FIRST_APP_TIMER;
/// A one-shot that collapses the island after the pointer left it (owned by the app's input).
pub const COLLAPSE_TIMER: usize = FIRST_APP_TIMER + 1;
/// Lands the clock tick just after the minute turns.
const CLOCK_SLACK_MS: u32 = 20;

/// The local time of day.
pub type Clock = Box<dyn Fn() -> Stamp>;

/// Milliseconds from `now` to just after the next minute boundary.
#[must_use]
pub fn clock_ms(now: Stamp) -> u32 {
    let into_minute = u32::from(now.second.min(59)) * 1000 + u32::from(now.millis.min(999));
    60_000 - into_minute + CLOCK_SLACK_MS
}

/// The tab bar text of `now`.
fn clock_of(now: Stamp) -> String {
    // Allowed: hours and minutes are below 60.
    #[allow(clippy::cast_possible_truncation)]
    clock_text(now.hour as u8, now.minute as u8)
}

/// Which timers run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Armed {
    /// The clock timer.
    pub clock: bool,
}

/// The timers `scene` wants. Open and shown, the clock runs on every page. Collapsed and shown,
/// it runs only for a look that shows the time (`look` is `None` before the idle wiring).
/// Suppressed, nothing runs (G2). The audio page needs no timer (the mixer's debounced signal
/// flags arrive with a wake) and neither does the media page (its elapsed time and seek are
/// live nodes).
#[must_use]
pub fn wanted(scene: &Scene, look: Option<IdleLook>) -> Armed {
    let resting = !scene.suppression.any() && look.is_some_and(shows_time);
    Armed {
        clock: is_open(scene) || resting,
    }
}

/// True for the looks that print the time.
fn shows_time(look: IdleLook) -> bool {
    matches!(look, IdleLook::Karot | IdleLook::Clock)
}

/// True when the island shows its pages: expanded and not suppressed.
fn is_open(scene: &Scene) -> bool {
    scene.expanded && !scene.suppression.any()
}

/// The part of a scene the feed reacts to.
type SceneKey = (bool, usize, Suppression);

fn key(scene: &Scene) -> SceneKey {
    (scene.expanded, scene.page, scene.suppression)
}

/// The services, the built pages and the timers.
pub struct Feed {
    services: Box<dyn Services>,
    timers: Box<dyn Timers>,
    clock: Clock,
    pages: Pages,
    facts: Facts,
    armed: Armed,
    clock_text: String,
    /// Pages a change reached while the island was closed; rebuilt when it opens (G2).
    pending: Dirty,
    /// The scene the feed last reacted to.
    seen: Option<SceneKey>,
    /// The audio page's pick mode.
    pick: PickMode,
    /// The mixer the pick was made in; a route write needs the same one.
    pick_mixer: Option<MixerShape>,
    /// The OSD sink and the last reading it saw.
    osd: osd::OsdFeed,
    /// The facts the welcome view is built from; `None` until the app wiring sets them, and
    /// the welcome slot then stays the placeholder.
    welcome: Option<WelcomeFacts>,
    /// The hour cell under the pointer on the welcome view.
    probe: Option<i32>,
    /// The idle look model; `None` keeps the seal pill and the placeholder welcome slot.
    idle: Option<Idle>,
    /// What the resting pill was last built from.
    idle_facts: Option<IdleFacts>,
    /// The weather service, when weather is on.
    weather: Option<WeatherLink>,
    /// The interface language, resolved once at start for every run mode.
    lang: Lang,
    /// The value a ruler drag shows until the reading catches up.
    drag: Option<drag::Override>,
    /// The settings window is open: its pill previews need the minute clock.
    sheet_open: bool,
    /// The shelf items, the hot slot and the saver.
    shelf: shelf::ShelfState,
}

impl Feed {
    /// Builds every page from whatever the services hold now (degraded while they start) and
    /// publishes it.
    #[must_use]
    pub fn new(
        mut services: Box<dyn Services>,
        timers: Box<dyn Timers>,
        clock: Clock,
        lang: Lang,
        mailbox: &SceneMailbox,
    ) -> Self {
        let _ = services.drain();
        let facts = services.facts();
        let clock_text = clock_of(clock());
        let pages = Pages::new(&Inputs {
            facts: &facts,
            pick: PickMode::None,
            clock: &clock_text,
            welcome: None,
            probe: None,
            shelf: ShelfView::default(),
            idle: None,
            lang,
        });
        let feed = Self {
            services,
            timers,
            clock,
            pages,
            facts,
            armed: Armed::default(),
            clock_text,
            pending: Dirty::default(),
            seen: None,
            pick: PickMode::None,
            pick_mixer: None,
            osd: osd::OsdFeed::default(),
            welcome: None,
            probe: None,
            idle: None,
            idle_facts: None,
            weather: None,
            lang,
            drag: None,
            sheet_open: false,
            shelf: shelf::ShelfState::default(),
        };
        feed.publish(mailbox);
        feed
    }

    /// Rebuilds the `dirty` lists from the current facts and publishes the content and, when a
    /// page height changed, the shapes. Equal content publishes nothing.
    fn rebuild(&mut self, dirty: Dirty, mailbox: &SceneMailbox) {
        if !dirty.any() {
            return;
        }
        let sources = self.facts.audio.vm.as_ref().map_or(0, |vm| vm.strips.len());
        self.pick = pick_within(self.pick, sources);
        if !route_allowed(self.pick_mixer, self.mixer()) {
            self.pick = PickMode::None;
        }
        let shown = self.shown_facts();
        let inputs = Inputs {
            facts: shown.as_ref().unwrap_or(&self.facts),
            pick: self.pick,
            clock: &self.clock_text,
            welcome: self.welcome.as_ref(),
            probe: self.probe,
            shelf: self.shelf.view(),
            idle: self
                .idle
                .as_ref()
                .zip(self.idle_facts.as_ref())
                .map(|(idle, facts)| (idle.look(), facts)),
            lang: self.lang,
        };
        self.pages.rebuild(dirty, &inputs);
        self.publish(mailbox);
    }

    /// The interface language this feed builds its pages in.
    #[must_use]
    pub fn lang(&self) -> Lang {
        self.lang
    }

    fn publish(&self, mailbox: &SceneMailbox) {
        let _ = mailbox.publish_content(std::sync::Arc::new(self.pages.content()));
        let shapes = self.pages.shapes();
        let _ = mailbox.update(|scene| Scene { shapes, ..scene });
    }

    /// A service published or another thread changed the scene: rebuild what changed and
    /// publish it. While closed only the pill is rebuilt; the pages wait for the opening.
    pub fn on_wake(&mut self, mailbox: &SceneMailbox) {
        let changes = self.services.drain();
        let forecast = self.take_forecast();
        if changes.any() || forecast {
            let facts = self.services.facts();
            let dirty = Dirty::between(&self.facts, &facts);
            let media = self.facts.media != facts.media;
            self.facts = facts;
            if media {
                self.observe_plays();
            }
            let dirty = dirty
                .union(self.refresh_idle())
                .union(self.settle_override());
            self.apply(dirty, mailbox);
        }
        // The first drain takes the baseline even when no change is reported; later drains
        // read only after a reported change.
        self.feed_osd(&mailbox.latest().scene, changes.any());
        if self.seen != Some(key(&mailbox.latest().scene)) {
            self.on_scene(mailbox);
        } else if changes.any() {
            self.sync_timers(mailbox);
        }
    }

    /// A UI timer ticked. A tick of a timer no longer armed (one queued before `KillTimer`)
    /// does nothing, and so does any tick the scene no longer wants (another publisher
    /// collapsed or suppressed it); that tick syncs the timers itself, so a lost wake cannot
    /// leave them running. Open, the tick refreshes the pages; collapsed, it moves the pill.
    pub fn on_timer(&mut self, id: usize, mailbox: &SceneMailbox) {
        let scene = mailbox.latest().scene;
        if !self.want(&scene).clock {
            self.sync_timers(mailbox);
            return;
        }
        if id != CLOCK_TIMER || !self.armed.clock {
            return;
        }
        // One-shot: the next tick is armed for the next minute below.
        self.timers.kill(CLOCK_TIMER);
        self.armed.clock = false;
        if is_open(&scene) {
            self.flush(mailbox);
        } else {
            self.minute(mailbox);
        }
        self.sync_timers(mailbox);
    }

    /// The scene changed (expanded, collapsed, suppressed): refresh the clock on opening and
    /// set the timers it wants.
    pub fn on_scene(&mut self, mailbox: &SceneMailbox) {
        let scene = mailbox.latest().scene;
        self.seen = Some(key(&scene));
        if is_open(&scene) {
            self.flush(mailbox);
        } else if !scene.suppression.any() {
            // Shown and resting: one catch-up draw of whatever moved while it was hidden.
            self.minute(mailbox);
        }
        self.sync_timers(mailbox);
    }

    /// Brings the pages up to date for an open island: the clock and the idle facts, then every
    /// page a change reached while closed, in one publish. The app calls it before it publishes
    /// an expand, so the first motion frames show current content.
    pub fn flush(&mut self, mailbox: &SceneMailbox) {
        let text = clock_of((self.clock)());
        if text != self.clock_text {
            self.clock_text = text;
            self.pending.pages = [true; PAGE_COUNT];
        }
        self.observe_plays();
        self.pending = self.pending.union(self.refresh_idle());
        let pending = std::mem::take(&mut self.pending);
        self.rebuild(pending, mailbox);
    }

    /// Stops the services and saves the play log; called once at shutdown, after the render
    /// thread was joined. The weather service is dropped, not joined: its worker may be inside
    /// a fetch, and a stop would block the UI thread for that long.
    pub fn stop(&mut self) {
        self.services.stop();
        self.observe_plays();
        if let Some(idle) = &self.idle {
            idle.save();
        }
        self.weather = None;
    }

    /// Starts and stops timers to match the scene.
    fn sync_timers(&mut self, mailbox: &SceneMailbox) {
        let want = self.want(&mailbox.latest().scene);
        match (want.clock, self.armed.clock) {
            (true, false) => self.timers.set(CLOCK_TIMER, clock_ms((self.clock)())),
            (false, true) => self.timers.kill(CLOCK_TIMER),
            _ => {}
        }
        self.armed = want;
    }

    /// The services, for the writes.
    pub fn services(&mut self) -> &mut dyn Services {
        self.services.as_mut()
    }

    /// The UI timers, for the app's collapse timer.
    pub fn timers(&self) -> &dyn Timers {
        self.timers.as_ref()
    }

    /// The facts the pages were last built from.
    #[must_use]
    pub fn facts(&self) -> &Facts {
        &self.facts
    }

    /// The hit targets of the last built pages, in tab order.
    #[must_use]
    pub fn targets(&self) -> &[Vec<HitTarget>; PAGE_COUNT] {
        self.pages.targets()
    }

    /// The audio page's pick mode.
    #[must_use]
    pub fn pick(&self) -> PickMode {
        self.pick
    }

    /// Sets the pick mode and rebuilds the audio page when it changed. A pick remembers the
    /// mixer it was made in.
    pub fn set_pick(&mut self, pick: PickMode, mailbox: &SceneMailbox) {
        if self.pick != pick {
            self.pick = pick;
            self.pick_mixer = match pick {
                PickMode::Picked(_) => self.mixer(),
                PickMode::None => None,
            };
            self.rebuild(Dirty::page(PageId::Audio), mailbox);
        }
    }

    /// The hour cell under the pointer on the welcome view.
    #[must_use]
    pub fn probe(&self) -> Option<i32> {
        self.probe
    }

    /// Ends the probe without a rebuild, for a collapse: the closed island builds nothing, so
    /// the welcome page waits for the opening.
    pub fn clear_probe(&mut self) {
        if self.probe.take().is_some() {
            self.pending.pages[PageId::Welcome.index()] = true;
        }
    }

    /// Sets the probe and rebuilds the welcome page only when it changed.
    pub fn set_probe(&mut self, probe: Option<i32>, mailbox: &SceneMailbox) {
        if self.probe != probe {
            self.probe = probe;
            self.rebuild(Dirty::page(PageId::Welcome), mailbox);
        }
    }

    /// The pointer is at `(x, y)` island DIP on the welcome view: sets the probe to the hour
    /// cell under it. Without welcome facts there is nothing to probe.
    pub fn move_probe(&mut self, x: f64, y: f64, mailbox: &SceneMailbox) {
        let probe = self.welcome.as_ref().and_then(|f| probe_hour(f, x, y));
        self.set_probe(probe, mailbox);
    }

    /// Sets the facts the welcome view is built from and rebuilds it. Until it is called, the
    /// welcome slot is the placeholder.
    pub fn set_welcome_facts(&mut self, facts: Option<WelcomeFacts>, mailbox: &SceneMailbox) {
        if self.welcome != facts {
            self.welcome = facts;
            self.rebuild(Dirty::page(PageId::Welcome), mailbox);
        }
    }

    /// The picked strip, if the services' mixer right now is the one the pick was made in.
    /// Read fresh from the services: a mixer restart may not have reached the pages yet.
    #[must_use]
    pub fn route_source(&self) -> Option<usize> {
        let PickMode::Picked(strip) = self.pick else {
            return None;
        };
        let now = MixerShape::of(self.services.facts().audio.vm.as_ref());
        route_allowed(self.pick_mixer, now).then_some(strip)
    }

    /// The shape of the mixer the pages were last built from.
    fn mixer(&self) -> Option<MixerShape> {
        MixerShape::of(self.facts.audio.vm.as_ref())
    }
}

#[cfg(test)]
mod tests;
