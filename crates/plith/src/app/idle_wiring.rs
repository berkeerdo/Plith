//! Builds the idle look model for a run and hands it to the feed: the settings (the file, or
//! the defaults in a probing run), the language, the play log and the weather service.

use super::feed::Feed;
use super::idle_facts::Idle;
use super::weather_link;
use crate::log::{self, LogFile};
use crate::services::fixture::idle as mock;
use crate::{locale, plays_file, settings_file};
use plith_core::lang::{Lang, resolve};
use plith_core::launch::{Fixture, Launch};
use plith_core::scene::SceneMailbox;
use plith_core::settings::Settings;
use plith_core::weather::Stamp;
use plith_shell::UiWaker;
use std::sync::Arc;

/// True for a normal run: real services, no demo, no idle check. Only a normal run reads and
/// writes the user's files and talks to the network; a probing run measures defaults.
#[must_use]
pub fn is_normal(launch: &Launch) -> bool {
    launch.demo.is_none() && launch.idle_check_secs.is_none() && launch.fixture.is_none()
}

/// The settings of this run: the file in a normal run, else the defaults, with `--idle-look`
/// on top. The banana fixture shows the mock's forecast, so its weather is on (no network).
#[must_use]
pub fn settings_for(launch: &Launch, log: &LogFile) -> Settings {
    let mut settings = if is_normal(launch) {
        settings_file::load(log)
    } else {
        Settings::default()
    };
    if let Some(look) = launch.idle_look {
        settings.look = look;
    }
    if launch.fixture == Some(Fixture::Banana) {
        settings.weather = true;
    }
    settings
}

/// The interface language of a run: the setting, or Windows' own for `Auto`. Resolved once at
/// start; the feed and the idle model share the result.
#[must_use]
pub fn language_of(settings: &Settings) -> Lang {
    resolve(&settings.lang, &locale::windows_language())
}

/// Attaches the idle model to `feed` and starts the weather service when this run allows it.
/// `settings` are the run's (`settings_for`); the language is the one the feed was built with.
pub fn attach(
    launch: &Launch,
    log: &Arc<LogFile>,
    waker: UiWaker,
    feed: &mut Feed,
    mailbox: &SceneMailbox,
    settings: &Settings,
) {
    let lang = feed.lang();
    let mut idle = Idle::new(settings.clone(), lang);
    // With plays=off nothing about listening is read, kept or written; the old file stays as is.
    if is_normal(launch) && settings.plays {
        idle = idle
            .with_plays(plays_file::load(log))
            .with_saver(plays_file::saver(Arc::clone(log)));
    }
    if launch.fixture == Some(Fixture::Banana) {
        let now = log::now();
        let day = (now.year, now.month, now.day);
        // Allowed: an hour is below 24.
        #[allow(clippy::cast_possible_truncation)]
        let read_at = Stamp {
            day,
            hour: now.hour as u8,
            minute: 0,
        };
        idle = idle
            .with_plays(mock::plays(day))
            .with_forecast(Some(mock::forecast(day, read_at)));
    }
    feed.set_idle(idle, mailbox);
    let link = weather_link::start(
        settings,
        is_normal(launch),
        log,
        Box::new(move || waker.wake()),
    );
    feed.set_weather(link);
}
