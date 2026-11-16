//! The feed on the UI thread: service readings become page content, and three UI timers keep
//! it current while the island is open. Collapsed, no timer runs (G2).

use crate::log::Stamp;
use crate::pages::{Dirty, Inputs, Pages};
use crate::services::{Facts, Services};
use plith_core::pages::audio::{Activity, PickMode};
use plith_core::pages::tabbar::clock_text;
use plith_core::pages::{HitTarget, PageId};
use plith_core::scene::{Scene, SceneMailbox, Suppression};
use plith_shell::{FIRST_APP_TIMER, TimerHandle};
use std::time::Instant;

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

/// Feeds `Activity` at 4 Hz while the audio page is open.
pub const ACTIVITY_TIMER: usize = FIRST_APP_TIMER;
/// Advances the elapsed time once a second while the media page is open and playing.
pub const MEDIA_TIMER: usize = FIRST_APP_TIMER + 1;
/// A one-shot to the next minute boundary for the tab bar clock while open.
pub const CLOCK_TIMER: usize = FIRST_APP_TIMER + 2;
/// A one-shot that collapses the island after the pointer left it (owned by the app's input).
pub const COLLAPSE_TIMER: usize = FIRST_APP_TIMER + 3;
/// The activity period.
pub const ACTIVITY_MS: u32 = 250;
/// The media period.
pub const MEDIA_MS: u32 = 1000;
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
    /// The activity timer.
    pub activity: bool,
    /// The media timer.
    pub media: bool,
    /// The clock timer.
    pub clock: bool,
}

/// The timers `scene` wants: none unless open and shown; the activity timer on the audio
/// page, the media timer on the media page while `playing`, the clock always.
#[must_use]
pub fn wanted(scene: &Scene, playing: bool) -> Armed {
    if !scene.expanded || scene.suppression.any() {
        return Armed::default();
    }
    let page = PageId::from_index(scene.page);
    Armed {
        activity: page == Some(PageId::Audio),
        media: page == Some(PageId::Media) && playing,
        clock: true,
    }
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
    activity: Activity,
    origin: Instant,
    armed: Armed,
    clock_text: String,
    /// Pages a change reached while the island was closed; rebuilt when it opens (G2).
    pending: Dirty,
    /// The scene the feed last reacted to.
    seen: Option<SceneKey>,
    /// The audio page's pick mode.
    pick: PickMode,
}

impl Feed {
    /// Builds every page from whatever the services hold now (degraded while they start) and
    /// publishes it.
    #[must_use]
    pub fn new(
        mut services: Box<dyn Services>,
        timers: Box<dyn Timers>,
        clock: Clock,
        mailbox: &SceneMailbox,
    ) -> Self {
        let _ = services.drain();
        let facts = services.facts(0.0);
        let clock_text = clock_of(clock());
        let activity = Activity::default();
        let pages = Pages::new(&Inputs {
            facts: &facts,
            activity: &activity,
            pick: PickMode::None,
            now_s: 0.0,
            clock: &clock_text,
        });
        let feed = Self {
            services,
            timers,
            clock,
            pages,
            facts,
            activity,
            origin: Instant::now(),
            armed: Armed::default(),
            clock_text,
            pending: Dirty::default(),
            seen: None,
            pick: PickMode::None,
        };
        feed.publish(mailbox);
        feed
    }

    fn now_s(&self) -> f64 {
        self.origin.elapsed().as_secs_f64()
    }

    /// Rebuilds the `dirty` lists from the current facts and publishes the content and, when a
    /// page height changed, the shapes. Equal content publishes nothing.
    fn rebuild(&mut self, dirty: Dirty, mailbox: &SceneMailbox) {
        if !dirty.any() {
            return;
        }
        let inputs = Inputs {
            facts: &self.facts,
            activity: &self.activity,
            pick: self.pick,
            now_s: self.now_s(),
            clock: &self.clock_text,
        };
        self.pages.rebuild(dirty, &inputs);
        self.publish(mailbox);
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
        if changes.any() {
            self.facts = self.services.facts(self.now_s());
            let dirty = Dirty::from_changes(changes);
            if is_open(&mailbox.latest().scene) {
                self.rebuild(dirty, mailbox);
            } else {
                for (wait, now) in self.pending.pages.iter_mut().zip(dirty.pages) {
                    *wait |= now;
                }
                self.rebuild(
                    Dirty {
                        pages: [false; 4],
                        pill: dirty.pill,
                    },
                    mailbox,
                );
            }
        }
        if self.seen != Some(key(&mailbox.latest().scene)) {
            self.on_scene(mailbox);
        } else if changes.any() {
            self.sync_timers(mailbox);
        }
    }

    /// A UI timer ticked. A tick of a timer no longer armed (one queued before `KillTimer`)
    /// does nothing, and so does any tick while the scene is not open (another publisher
    /// collapsed it and its wake, which syncs the timers, is still queued).
    pub fn on_timer(&mut self, id: usize, mailbox: &SceneMailbox) {
        if !is_open(&mailbox.latest().scene) {
            return;
        }
        let armed = match id {
            ACTIVITY_TIMER => self.armed.activity,
            MEDIA_TIMER => self.armed.media,
            CLOCK_TIMER => self.armed.clock,
            _ => false,
        };
        if !armed {
            return;
        }
        match id {
            ACTIVITY_TIMER => self.read_activity(mailbox),
            MEDIA_TIMER => {
                self.facts = self.services.facts(self.now_s());
                self.rebuild(Dirty::page(PageId::Media), mailbox);
            }
            CLOCK_TIMER => {
                // One-shot: the next tick is armed for the next minute below.
                self.timers.kill(CLOCK_TIMER);
                self.armed.clock = false;
                self.flush(mailbox);
            }
            _ => {}
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
        }
        self.sync_timers(mailbox);
    }

    /// Brings the pages up to date for an open island: the clock, then every page a change
    /// reached while closed, in one publish. The app calls it before it publishes an expand,
    /// so the first motion frames show current content.
    pub fn flush(&mut self, mailbox: &SceneMailbox) {
        let text = clock_of((self.clock)());
        if text != self.clock_text {
            self.clock_text = text;
            self.pending.pages = [true; 4];
        }
        let pending = std::mem::take(&mut self.pending);
        self.rebuild(pending, mailbox);
    }

    /// Stops the services; called once at shutdown, after the render thread was joined.
    pub fn stop(&mut self) {
        self.services.stop();
    }

    /// Feeds the current levels to the activity and rebuilds the audio page.
    fn read_activity(&mut self, mailbox: &SceneMailbox) {
        if let Some(levels) = self.services.levels() {
            self.activity.observe(&levels.strips, self.now_s());
            self.rebuild(Dirty::page(PageId::Audio), mailbox);
        }
    }

    /// Starts and stops timers to match the scene; a newly started activity timer reads the
    /// levels at once, so the routes light without waiting a tick.
    fn sync_timers(&mut self, mailbox: &SceneMailbox) {
        let want = wanted(&mailbox.latest().scene, self.facts.media.playing);
        let have = self.armed;
        let clock_ms = clock_ms((self.clock)());
        for (on, was, id, ms) in [
            (want.activity, have.activity, ACTIVITY_TIMER, ACTIVITY_MS),
            (want.media, have.media, MEDIA_TIMER, MEDIA_MS),
            (want.clock, have.clock, CLOCK_TIMER, clock_ms),
        ] {
            match (on, was) {
                (true, false) => self.timers.set(id, ms),
                (false, true) => self.timers.kill(id),
                _ => {}
            }
        }
        self.armed = want;
        if want.activity && !have.activity {
            self.read_activity(mailbox);
        }
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
    pub fn targets(&self) -> &[Vec<HitTarget>; 4] {
        self.pages.targets()
    }

    /// The audio page's pick mode.
    #[must_use]
    pub fn pick(&self) -> PickMode {
        self.pick
    }

    /// Sets the pick mode and rebuilds the audio page when it changed.
    pub fn set_pick(&mut self, pick: PickMode, mailbox: &SceneMailbox) {
        if self.pick != pick {
            self.pick = pick;
            self.rebuild(Dirty::page(PageId::Audio), mailbox);
        }
    }
}
