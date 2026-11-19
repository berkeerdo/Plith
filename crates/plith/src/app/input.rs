//! Pointer input on the UI thread: clicks become page actions through the services, and the
//! pointer leaving the open island collapses it after a delay.

use super::feed::{COLLAPSE_TIMER, Feed};
use super::state::App;
use plith_core::demo::DemoStep;
use plith_core::input::{COLLAPSE_DELAY_S, ClickResult, route_click, toggle_pick};
use plith_core::page::PageLayers;
use plith_core::pages::audio::PickMode;
use plith_core::pages::{Action, HitTarget, PAGE_COUNT, PageId};
use plith_core::scene::{Scene, SceneMailbox};
use std::time::Duration;

/// The collapse delay as the UI timer period.
// Allowed: 0.4 s is 400 ms, exact and far inside u32.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
const COLLAPSE_MS: u32 = (COLLAPSE_DELAY_S * 1000.0) as u32;

impl App {
    /// A click at `(x, y)` island DIP: expands a collapsed island, runs the action of the page
    /// target under it, or does nothing on empty page space.
    pub(super) fn on_click(&mut self, x: f64, y: f64) {
        let expanded = self.mailbox.latest().scene.expanded;
        let layers = self
            .shapes
            .latest()
            .map_or_else(PageLayers::empty, |frame| frame.pages);
        let none: [Vec<HitTarget>; PAGE_COUNT] = Default::default();
        let targets = self.feed.as_ref().map_or(&none, Feed::targets);
        match route_click(expanded, &layers, targets, x, y) {
            ClickResult::Expand => self.expand(),
            ClickResult::Action(Action::Tab(page)) => self.turn_to(page),
            ClickResult::Action(action) => {
                if let Some(feed) = self.feed.as_mut() {
                    dispatch(feed, &self.mailbox, action);
                }
            }
            ClickResult::Nothing => {}
        }
    }

    /// Opens the island with the pages brought up to date first: on the welcome view, or on the
    /// page shown last when the settings say so.
    fn expand(&mut self) {
        let page = self.feed.as_ref().and_then(Feed::open_page);
        self.expand_on(page);
    }

    /// Opens the island (on `page`, when given) with the pages brought up to date first, in
    /// one scene publish after the content publish. Opening on another page ends a pick.
    fn expand_on(&mut self, page: Option<PageId>) {
        self.cancel_collapse();
        if let Some(feed) = &mut self.feed {
            if page.is_some_and(|p| p != PageId::Audio) {
                feed.set_pick(PickMode::None, &self.mailbox);
            }
            feed.flush(&self.mailbox);
        }
        let _ = self.mailbox.update(|scene| Scene {
            expanded: true,
            page: page.map_or(scene.page, PageId::index),
            ..scene
        });
        self.scene_changed();
    }

    /// Applies the demo's queued scene steps through the click paths: expand and page steps
    /// flush before they publish, as a click does.
    pub(super) fn apply_demo_steps(&mut self) {
        let steps: Vec<DemoStep> = match &self.demo {
            Some(rx) => rx.try_iter().collect(),
            None => return,
        };
        for step in steps {
            match step {
                // A demo expands on the page shown, so its measurements stay comparable.
                DemoStep::Expand => self.expand_on(None),
                DemoStep::Page(index) => {
                    let Some(page) = PageId::from_index(index) else {
                        continue;
                    };
                    if self.mailbox.latest().scene.expanded {
                        self.turn_to(page);
                    } else {
                        self.expand_on(Some(page));
                    }
                }
                DemoStep::Collapse => self.collapse_now(),
                DemoStep::Wait(_) | DemoStep::Content(_) => {}
            }
        }
    }

    /// Turns to `page`; leaving the audio page ends a pick.
    fn turn_to(&mut self, page: PageId) {
        if let (true, Some(feed)) = (page != PageId::Audio, self.feed.as_mut()) {
            feed.set_pick(PickMode::None, &self.mailbox);
        }
        let _ = self.mailbox.update(|scene| Scene {
            page: page.index(),
            ..scene
        });
        self.scene_changed();
    }

    /// The pointer moved inside the island shape. Only an open island showing the welcome view
    /// reads it: the hour under the pointer becomes the probe. No timer, no hover-open.
    pub(super) fn on_move(&mut self, x: f64, y: f64) {
        let scene = self.mailbox.latest().scene;
        if !scene.expanded || scene.page != PageId::Welcome.index() {
            return;
        }
        if let Some(feed) = self.feed.as_mut() {
            feed.move_probe(x, y, &self.mailbox);
        }
    }

    /// The pointer left the window: the probe ends, and an open island starts the collapse
    /// delay.
    pub(super) fn on_leave(&mut self) {
        if let Some(feed) = self.feed.as_mut()
            && feed.probe().is_some()
        {
            feed.set_probe(None, &self.mailbox);
        }
        if !self.mailbox.latest().scene.expanded {
            return;
        }
        self.collapse.leave((self.now)());
        if let Some(feed) = &self.feed {
            feed.timers().set(COLLAPSE_TIMER, COLLAPSE_MS);
        }
    }

    /// The pointer is back: no collapse.
    pub(super) fn on_enter(&mut self) {
        self.cancel_collapse();
    }

    fn cancel_collapse(&mut self) {
        if self.collapse.leave_at.is_none() {
            return;
        }
        self.collapse.enter();
        if let Some(feed) = &self.feed {
            feed.timers().kill(COLLAPSE_TIMER);
        }
    }

    /// The collapse timer ticked. Stray ticks (the pointer came back) stop it; an early tick
    /// waits for the rest of the delay; a due tick collapses an expanded island (a hidden one
    /// too: it then shows collapsed when the suppression ends) and ends a pick.
    pub(super) fn on_collapse_tick(&mut self) {
        let now = (self.now)();
        let remaining = self.collapse.remaining(now);
        let Some(feed) = self.feed.as_mut() else {
            return;
        };
        if let (Some(left), false) = (remaining, self.collapse.due(now)) {
            // Allowed: `left` is below the 0.4 s delay, a few hundred ms.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let ms = (left * 1000.0).ceil().max(1.0) as u32;
            feed.timers().set(COLLAPSE_TIMER, ms);
            return;
        }
        feed.timers().kill(COLLAPSE_TIMER);
        if remaining.is_none() {
            return;
        }
        self.collapse.enter();
        self.collapse_now();
    }

    /// Collapses an expanded island now and ends a pick.
    fn collapse_now(&mut self) {
        if !self.mailbox.latest().scene.expanded {
            return;
        }
        if let Some(feed) = self.feed.as_mut() {
            feed.set_pick(PickMode::None, &self.mailbox);
            feed.clear_probe();
        }
        let _ = self.mailbox.update(|scene| Scene {
            expanded: false,
            ..scene
        });
        self.scene_changed();
    }
}

pub(super) mod drag;
#[cfg(test)]
mod drag_tests;

#[cfg(test)]
mod tests;

/// Runs a page action other than a tab: pick mode changes on the feed, writes through the
/// services (instant on the fixture, on the writes thread for the hardware).
fn dispatch(feed: &mut Feed, mailbox: &SceneMailbox, action: Action) {
    match action {
        Action::PickSource(source) => {
            let next = toggle_pick(feed.pick(), source);
            feed.set_pick(next, mailbox);
        }
        Action::PickOutput(bus) => {
            // A pick made in another mixer edition ends without a write.
            if let Some(strip) = feed.route_source() {
                feed.services().toggle_route(strip, bus);
            }
            feed.set_pick(PickMode::None, mailbox);
        }
        Action::CancelPick => feed.set_pick(PickMode::None, mailbox),
        Action::SetDefaultEndpoint(id) => feed.services().set_default_endpoint(&id),
        Action::ToggleMic => feed.services().toggle_mic(),
        Action::SetVolume(percent) => feed.services().set_volume(percent),
        Action::SetBrightness(percent) => feed.services().set_brightness(percent),
        Action::PlayPause => feed.services().play_pause(),
        Action::Next => feed.services().next(),
        Action::Previous => feed.services().previous(),
        Action::Seek(fraction) => {
            let Some(timeline) = feed.facts().media.timeline else {
                return;
            };
            let seconds = timeline.duration_s * fraction.clamp(0.0, 1.0);
            if let Ok(position) = Duration::try_from_secs_f64(seconds) {
                feed.services().seek(position);
            }
        }
        // `hit` resolves the rulers and the app handles tabs before dispatch.
        Action::Tab(_)
        | Action::SeekRuler(_)
        | Action::BrightnessRuler(_)
        | Action::VolumeRuler(_) => {}
    }
}
