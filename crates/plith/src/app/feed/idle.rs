//! The idle side of the feed: the resting look, the welcome facts, the play log and the weather.
//! Everything here runs on an event (a wake, the minute tick, a scene change); nothing arms a
//! timer of its own.

use super::{Dirty, Feed, PAGE_COUNT, WeatherLink, is_open};
use crate::app::idle_facts::Idle;
use plith_core::pages::PageId;
use plith_core::pages::idle_pill::{IdleFacts, rest_shape};
use plith_core::scene::{Scene, SceneMailbox};
use plith_core::settings::{IdleLook, OpenOn};

impl Feed {
    /// Attaches the idle look model: publishes the resting shape, builds the pill and the
    /// welcome view from it and sets the timers it needs.
    pub fn set_idle(&mut self, idle: Idle, mailbox: &SceneMailbox) {
        let rest = rest_shape(idle.look());
        self.idle = Some(idle);
        let _ = mailbox.update(|scene| Scene { rest, ..scene });
        self.observe_plays();
        let now = (self.clock)();
        let (pill, welcome) = match self.idle.as_ref() {
            Some(idle) => (
                idle.idle_facts(&self.facts, now),
                idle.welcome_facts(&self.facts, now),
            ),
            None => return,
        };
        self.idle_facts = Some(pill);
        self.set_welcome_facts(Some(welcome), mailbox);
        self.rebuild(
            Dirty {
                pages: [false; PAGE_COUNT],
                pill: true,
            }
            .with_quick(),
            mailbox,
        );
        self.sync_timers(mailbox);
    }

    /// Hands the feed the weather service it reads forecasts from.
    pub fn set_weather(&mut self, weather: Option<WeatherLink>) {
        self.weather = weather;
    }

    /// What the resting pill was last built from; the settings window's previews follow it.
    #[must_use]
    pub fn idle_facts(&self) -> Option<&IdleFacts> {
        self.idle_facts.as_ref()
    }

    /// The resting look; `None` before the idle model is attached.
    pub(super) fn look(&self) -> Option<IdleLook> {
        self.idle.as_ref().map(Idle::look)
    }

    /// The page an expand lands on: the welcome view, or `None` for the page shown last (also
    /// before the idle model is attached).
    #[must_use]
    pub fn open_page(&self) -> Option<PageId> {
        match self.idle.as_ref()?.open_on() {
            OpenOn::Welcome => Some(PageId::Welcome),
            OpenOn::LastPage => None,
        }
    }

    /// Feeds the play log the current media reading.
    pub(super) fn observe_plays(&mut self) {
        let now = (self.clock)();
        if let Some(idle) = self.idle.as_mut() {
            idle.observe(&self.facts, now);
        }
    }

    /// The pointer is over the resting pill: a finished title scroll starts again, and the pill
    /// is republished only when that changed its list.
    pub fn hover_pill(&mut self, mailbox: &SceneMailbox) {
        let restarted = self
            .idle
            .as_mut()
            .is_some_and(|idle| idle.restart_scroll(&self.facts));
        if restarted {
            let dirty = self.refresh_idle();
            self.apply(dirty, mailbox);
        }
    }

    /// A forecast that arrived since the last call goes to the model; true when it is new.
    pub(super) fn take_forecast(&mut self) -> bool {
        let Some(forecast) = self.weather.as_mut().and_then(WeatherLink::take) else {
            return false;
        };
        self.idle
            .as_mut()
            .is_some_and(|idle| idle.set_forecast(forecast))
    }

    /// Rebuilds the idle facts from the model and says which lists they changed.
    pub(super) fn refresh_idle(&mut self) -> Dirty {
        let Some(idle) = self.idle.as_ref() else {
            return Dirty::default();
        };
        let now = (self.clock)();
        let pill = Some(idle.idle_facts(&self.facts, now));
        let welcome = Some(idle.welcome_facts(&self.facts, now));
        let mut dirty = Dirty {
            pill: self.idle_facts != pill,
            ..Dirty::default()
        };
        dirty.pages[PageId::Welcome.index()] = self.welcome != welcome;
        dirty = dirty.with_quick();
        self.idle_facts = pill;
        self.welcome = welcome;
        dirty
    }

    /// Rebuilds `dirty` for an open island. A closed one rebuilds only the pill and its pages wait
    /// for the opening (G2). A suppressed one rebuilds nothing, not even the pill: the whole of
    /// `dirty` waits in `pending` for the catch-up when it is shown again.
    pub(super) fn apply(&mut self, dirty: Dirty, mailbox: &SceneMailbox) {
        let scene = mailbox.latest().scene;
        if is_open(&scene) {
            self.rebuild(dirty, mailbox);
            return;
        }
        if scene.suppression.any() {
            self.pending = self.pending.union(dirty);
            return;
        }
        // Shown and resting: the pill also takes what waited while it was hidden.
        let pill = dirty.pill || self.pending.pill;
        self.pending = self.pending.union(Dirty {
            pill: false,
            ..dirty
        });
        self.pending.pill = false;
        self.rebuild(
            Dirty {
                pages: [false; PAGE_COUNT],
                pill,
            },
            mailbox,
        );
    }

    /// The resting update: the minute tick of a time look, and the one catch-up draw when the
    /// island is shown again.
    pub(super) fn minute(&mut self, mailbox: &SceneMailbox) {
        self.observe_plays();
        let dirty = self.refresh_idle();
        self.apply(dirty, mailbox);
    }
}
