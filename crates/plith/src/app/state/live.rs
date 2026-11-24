//! Settings applied while running: the feed (language, idle look, welcome view), the weather
//! link and the OSD side. Nothing here blocks the UI thread: weather and OSD stops run on short
//! threads of their own.

use super::App;
use crate::app::idle_wiring::language_of;
use crate::app::osd_slot::{OsdSlot, RealOsd};
use crate::app::saver::Saver;
use crate::app::weather_link::WeatherWiring;
use crate::plays_file;
use plith_core::launch::{FlyoutMode, flyout_mode};
use plith_core::settings::change::{Effects, SettingChange, apply_change};
use plith_core::settings::{IdleLook, Settings};
use plith_core::theme::Theme;
use std::sync::Arc;

/// The OSD slot of a normal run and the `--flyout` that wins over the settings file.
pub struct OsdControl {
    slot: OsdSlot<RealOsd>,
    cli: Option<FlyoutMode>,
}

impl OsdControl {
    /// Whether `--flyout` is on the command line (it wins over the setting).
    #[must_use]
    pub fn flyout_locked(&self) -> bool {
        self.cli.is_some()
    }

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

    /// Takes the settings of the run; the app owns them from here on.
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Saves every later change through `saver`; a normal run only.
    pub fn attach_saver(&mut self, saver: Saver) {
        self.saver = Some(saver);
    }

    /// Writes the settings still queued and ends the saver, at exit.
    pub fn flush_saver(&mut self) {
        if let Some(saver) = self.saver.take() {
            saver.flush_and_join();
        }
    }

    /// At exit: the settings window first (a city still being typed is committed), then the
    /// file.
    pub fn finish_settings(&mut self) {
        self.close_sheet_with(false);
        self.flush_saver();
    }

    /// Whether the last save failed.
    pub(super) fn save_failed(&self) -> bool {
        self.saver.as_ref().is_some_and(Saver::failed)
    }

    /// Remembers the look the settings file holds while `--idle-look` shows another one for
    /// this run, so saves keep the file's value until the user picks a look.
    pub fn keep_file_look(&mut self, look: Option<IdleLook>) {
        self.file_look = look;
    }

    /// Remembers the theme the settings file holds while `--theme` shows another one for this
    /// run, so saves keep the file's value until the user picks a theme.
    pub fn keep_file_theme(&mut self, theme: Option<Theme>) {
        self.file_theme = theme;
    }

    /// The one path of a setting change, from the settings window or the quick page: update
    /// the settings, apply what changed while running, then save. A value already set does
    /// nothing.
    pub fn change(&mut self, change: SettingChange) {
        self.change_with(change, true);
    }

    /// Commits a city typed in the window at exit: saved, not applied (the weather is going
    /// away).
    pub(super) fn save_change(&mut self, change: SettingChange) {
        self.change_with(change, false);
    }

    /// The change path; `apply` false only records and saves the change.
    fn change_with(&mut self, change: SettingChange, apply: bool) {
        // Picking a look ends the command line's override, even to the look already shown.
        let mut unpinned =
            matches!(change, SettingChange::Look(_)) && self.file_look.take().is_some();
        // The same for the theme and `--theme`.
        unpinned |= matches!(change, SettingChange::Theme(_)) && self.file_theme.take().is_some();
        let effects = apply_change(&mut self.settings, change);
        // Every field that changed reports at least one effect.
        if effects == Effects::default() && !unpinned {
            return;
        }
        let settings = self.settings.clone();
        if apply {
            self.apply_settings(&settings, effects);
        }
        if let Some(saver) = &self.saver {
            let mut saved = settings;
            if let Some(look) = self.file_look {
                saved.look = look;
            }
            if let Some(theme) = self.file_theme {
                saved.theme = theme;
            }
            saver.save(saved);
        }
        self.refresh_sheet();
    }

    /// Applies `settings` after a change with `effects` (see `apply_change`): the feed, then
    /// the weather link, then the OSD side.
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
        if effects.lang {
            self.sync_tray(language_of(settings));
        }
        if effects.osd {
            self.apply_osd(settings);
        }
        // The pages' lists name colour roles, not colours: the render thread draws them again
        // in the new palette. The settings window takes the theme from its facts on refresh.
        if effects.theme
            && let Some(relayout) = &self.relayout
        {
            relayout.set_palette(settings.theme.palette());
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

#[cfg(test)]
mod tests;
