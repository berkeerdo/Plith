//! Settings applied while running: the feed (language, idle look, welcome view), the weather
//! link and the OSD side. Nothing here blocks the UI thread: weather and OSD stops run on short
//! threads of their own.

use super::App;
use crate::app::idle_wiring::language_of;
use crate::app::osd_slot::{OsdSlot, RealOsd};
use crate::app::weather_link::WeatherWiring;
use crate::plays_file;
use plith_core::launch::{FlyoutMode, flyout_mode};
use plith_core::settings::Settings;
use plith_core::settings::change::Effects;
use std::sync::Arc;

/// The OSD slot of a normal run and the `--flyout` that wins over the settings file.
pub struct OsdControl {
    slot: OsdSlot<RealOsd>,
    cli: Option<FlyoutMode>,
}

impl OsdControl {
    /// Wraps `slot`; `cli` is the command line's flyout mode, if any.
    #[must_use]
    pub fn new(slot: OsdSlot<RealOsd>, cli: Option<FlyoutMode>) -> Self {
        Self { slot, cli }
    }
}

impl App {
    /// Keeps the wiring that starts the weather again when the user turns it on. `normal` is
    /// true in a normal run, the only one that reads and writes the play log.
    pub fn attach_weather(&mut self, wiring: WeatherWiring, normal: bool) {
        self.weather = Some(wiring);
        self.normal_run = normal;
    }

    /// Takes the OSD slot and switches it to `settings`; a normal run only.
    pub fn attach_osd(&mut self, osd: OsdControl, settings: &Settings) {
        self.osd = Some(osd);
        self.apply_osd(settings);
    }

    /// Applies `settings` after a change with `effects` (see `apply_change`): the feed, then
    /// the weather link, then the OSD side.
    #[expect(dead_code, reason = "the settings window calls it (plan Task 10)")]
    pub fn apply_settings(&mut self, settings: &Settings, effects: Effects) {
        if let Some(feed) = &mut self.feed {
            // Only a normal run reads and writes the play log.
            if settings.plays && self.normal_run {
                let log = Arc::clone(&self.log);
                feed.enable_plays(move || (plays_file::load(&log), plays_file::saver(log)));
            }
            let lang = if effects.lang {
                language_of(settings)
            } else {
                feed.lang()
            };
            feed.apply_settings(settings, lang, effects, &self.mailbox);
            if effects.weather
                && let Some(wiring) = &self.weather
            {
                let link = feed.take_weather();
                feed.set_weather(wiring.apply(link, settings));
            }
        }
        if effects.osd {
            self.apply_osd(settings);
        }
    }

    /// Switches the OSD side to `settings` and hands a new side's sink to the feed.
    fn apply_osd(&mut self, settings: &Settings) {
        if let Some(osd) = &mut self.osd {
            let mode = flyout_mode(osd.cli, settings.flyout);
            osd.slot.apply(settings.osd, mode);
        }
        self.hand_osd_sink();
    }

    /// On a wake: a finished OSD stop starts the pending side.
    pub(super) fn poll_osd(&mut self) {
        if let Some(osd) = &mut self.osd {
            osd.slot.poll();
        }
        self.hand_osd_sink();
    }

    fn hand_osd_sink(&mut self) {
        let sink = self.osd.as_mut().and_then(|osd| osd.slot.take_sink());
        if let Some(sink) = sink {
            self.set_osd(sink);
        }
    }

    /// Stops the OSD side at exit: suppression off and flyouts shown, then the link sends
    /// `Bye`. A side still stopping is joined for at most `STOP_WAIT`.
    pub fn stop_osd(&mut self) {
        if let Some(osd) = &mut self.osd {
            osd.slot.shutdown();
        }
    }
}
