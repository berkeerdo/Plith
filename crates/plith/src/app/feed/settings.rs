//! Settings changed while running: the language, the idle look and the welcome view follow at
//! once, without a collapse or a page change.

use super::{Dirty, Feed, PAGE_COUNT, WeatherLink};
use plith_core::lang::Lang;
use plith_core::pages::PageId;
use plith_core::pages::idle_pill::rest_shape;
use plith_core::scene::{Scene, SceneMailbox};
use plith_core::settings::Settings;
use plith_core::settings::change::Effects;

impl Feed {
    /// Takes `settings` and `lang` (resolved by the caller) and rebuilds what `effects` names:
    /// every page and the pill for a language switch, the welcome view and the pill otherwise,
    /// plus whatever the new idle facts changed. The scene's page and expansion stay; a closed
    /// island rebuilds the pill now and the pages when it opens (G2). The OSD word follows on
    /// the next level, which reads the feed's language.
    pub fn apply_settings(
        &mut self,
        settings: &Settings,
        lang: Lang,
        effects: Effects,
        mailbox: &SceneMailbox,
    ) {
        self.lang = lang;
        let look_before = self.look();
        if let Some(idle) = self.idle.as_mut() {
            idle.set_settings(settings.clone());
            idle.set_lang(lang);
        }
        if let Some(look) = self.look().filter(|look| Some(*look) != look_before) {
            let rest = rest_shape(look);
            let _ = mailbox.update(|scene| Scene { rest, ..scene });
        }
        let named = if effects.lang {
            Dirty {
                pages: [true; PAGE_COUNT],
                pill: true,
            }
        } else {
            let mut dirty = Dirty {
                pill: effects.pill,
                ..Dirty::default()
            };
            dirty.pages[PageId::Welcome.index()] = effects.welcome;
            dirty
        };
        let dirty = named.union(self.refresh_idle());
        self.apply(dirty, mailbox);
        self.sync_timers(mailbox);
    }

    /// Takes the weather link out, to reconfigure, stop or replace it.
    #[expect(dead_code, reason = "the settings window reaches it (plan Task 10)")]
    pub fn take_weather(&mut self) -> Option<WeatherLink> {
        self.weather.take()
    }
}

#[cfg(test)]
mod tests;
