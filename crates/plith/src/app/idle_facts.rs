//! The idle look model: the settings, the play log and the forecast, turned into the facts the
//! resting pill and the welcome view are built from. Pure over its inputs; the feed decides
//! when to ask.

use crate::log::Stamp;
use crate::services::Facts;
use plith_core::lang::{Lang, Text, date_short, device_kind, text, weekday};
use plith_core::pages::idle_pill::{
    IdleFacts, SoundWords, sound_words, title_run_s, weather_words,
};
use plith_core::pages::pill::PillState;
use plith_core::pages::tabbar::clock_text;
use plith_core::pages::welcome::{SoundHead, WelcomeFacts};
use plith_core::plays::{Day, PlayLog};
use plith_core::settings::{IdleLook, OpenOn, Settings};
use plith_core::weather::{Forecast, Stamp as WeatherStamp};
use std::time::SystemTime;

/// Writes the play log to disk.
pub type PlaySaver = Box<dyn Fn(&PlayLog)>;

/// The model behind the resting pill and the welcome view.
pub struct Idle {
    settings: Settings,
    lang: Lang,
    plays: PlayLog,
    forecast: Option<Forecast>,
    /// The track playing at the last observation, to see when a play closes.
    playing: Option<(String, String)>,
    saver: Option<PlaySaver>,
    /// The last track the title scroll was started for; a pause keeps it.
    scroll_track: Option<(String, String)>,
    /// When the title's one-shot scroll last began.
    scroll_start: Option<SystemTime>,
    /// The wall clock the scroll start reads.
    wall: Box<dyn Fn() -> SystemTime>,
}

/// The minute of the day and the second of `now`.
fn clock_of(now: Stamp) -> (u32, u32) {
    (
        u32::from(now.hour) * 60 + u32::from(now.minute),
        u32::from(now.second),
    )
}

fn day_of(now: Stamp) -> Day {
    (now.year, now.month, now.day)
}

impl Idle {
    /// A model with no plays and no forecast yet.
    #[must_use]
    pub fn new(settings: Settings, lang: Lang) -> Self {
        Self {
            settings,
            lang,
            plays: PlayLog::default(),
            forecast: None,
            playing: None,
            saver: None,
            scroll_track: None,
            scroll_start: None,
            wall: Box::new(SystemTime::now),
        }
    }

    /// Starts from `plays` (the saved log).
    #[must_use]
    pub fn with_plays(self, plays: PlayLog) -> Self {
        Self { plays, ..self }
    }

    /// Starts with `forecast`.
    #[must_use]
    pub fn with_forecast(self, forecast: Option<Forecast>) -> Self {
        Self { forecast, ..self }
    }

    /// Calls `saver` when a play closes and at exit.
    #[must_use]
    pub fn with_saver(self, saver: PlaySaver) -> Self {
        Self {
            saver: Some(saver),
            ..self
        }
    }

    /// Replaces the wall clock the scroll start reads.
    #[cfg(test)]
    #[must_use]
    pub fn with_wall(self, wall: Box<dyn Fn() -> SystemTime>) -> Self {
        Self { wall, ..self }
    }

    /// The resting look.
    #[must_use]
    pub fn look(&self) -> IdleLook {
        self.settings.look
    }

    /// Where an expand lands.
    #[must_use]
    pub fn open_on(&self) -> OpenOn {
        self.settings.open_on
    }

    /// Takes the settings the user changed while running.
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Takes the interface language the user switched to while running.
    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
    }

    /// Plays turned on while running in a run that started with them off: `load` gives the
    /// saved log and its saver, once. True when it loaded; a model that has a saver keeps it.
    pub fn enable_plays(&mut self, load: impl FnOnce() -> (PlayLog, PlaySaver)) -> bool {
        if self.saver.is_some() {
            return false;
        }
        let (plays, saver) = load();
        self.plays = plays;
        self.saver = Some(saver);
        true
    }

    /// Forgets the forecast: weather went off, or the place changed. The views show no
    /// temperature until the next fetch.
    pub fn clear_forecast(&mut self) {
        self.forecast = None;
    }

    /// Takes a new forecast; true when it differs from the one held.
    pub fn set_forecast(&mut self, forecast: Forecast) -> bool {
        let changed = self.forecast.as_ref() != Some(&forecast);
        self.forecast = Some(forecast);
        changed
    }

    /// Feeds the play log one media reading. The log is saved when the track that was playing
    /// ended (a skip, a pause or a stop), never on every call: an open play under ten seconds
    /// is not a play yet. With `plays=off` it does nothing: no history is kept.
    pub fn observe(&mut self, facts: &Facts, now: Stamp) {
        let track = playing_track(facts);
        self.note_track(track.as_ref());
        if !self.settings.plays {
            return;
        }
        let (minute, second) = clock_of(now);
        self.plays.observe(
            day_of(now),
            minute,
            second,
            track.as_ref().map(|(t, a)| (t.as_str(), a.as_str())),
            track.is_some(),
        );
        if self.playing != track {
            let closed = self.playing.is_some();
            self.playing = track;
            if closed {
                self.save();
            }
        }
    }

    /// A new playing track starts the title scroll; the same track keeps its start, and a pause
    /// and resume of it is no change.
    fn note_track(&mut self, track: Option<&(String, String)>) {
        if track.is_some() && track != self.scroll_track.as_ref() {
            self.scroll_track = track.cloned();
            self.scroll_start = Some((self.wall)());
        }
    }

    /// The pointer is over the resting pill: starts the title scroll again when the title
    /// scrolls at all and no run is under way (`start <= now < start + run`; a start in the
    /// future after the wall clock stepped back is no run). True when the start changed.
    pub fn restart_scroll(&mut self, facts: &Facts) -> bool {
        let Some((title, _)) = playing_track(facts) else {
            return false;
        };
        let Some(run_s) = title_run_s(&title) else {
            return false;
        };
        let now = (self.wall)();
        let running = self.scroll_start.is_some_and(|start| {
            now.duration_since(start)
                .is_ok_and(|elapsed| elapsed.as_secs_f64() < run_s)
        });
        if running {
            return false;
        }
        self.scroll_start = Some(now);
        true
    }

    /// Writes the play log now (at exit and when a play closes).
    pub fn save(&self) {
        if !self.settings.plays {
            return;
        }
        if let Some(saver) = &self.saver {
            saver(&self.plays);
        }
    }

    fn weather_stamp(now: Stamp) -> WeatherStamp {
        // Allowed: an hour and a minute are below 60.
        #[allow(clippy::cast_possible_truncation)]
        WeatherStamp {
            day: day_of(now),
            hour: now.hour as u8,
            minute: now.minute as u8,
        }
    }

    /// What the resting pill is built from.
    #[must_use]
    pub fn idle_facts(&self, facts: &Facts, now: Stamp) -> IdleFacts {
        let s = &self.settings;
        let words = self
            .forecast
            .as_ref()
            .filter(|_| s.weather)
            .map(|f| weather_words(f, Self::weather_stamp(now), s.unit, self.lang))
            .unwrap_or_default();
        // Allowed: an hour and a minute are below 60.
        #[allow(clippy::cast_possible_truncation)]
        let time = clock_text(now.hour as u8, now.minute as u8);
        IdleFacts {
            time,
            turn: words.turn,
            now_sky: words.now_sky,
            playing: playing_track(facts),
            seal: facts.pill.clone(),
            sound: pill_sound(facts, self.lang),
            lang: self.lang,
            scroll_start: self.scroll_start,
        }
    }

    /// What the welcome view is built from.
    #[must_use]
    pub fn welcome_facts(&self, facts: &Facts, now: Stamp) -> WelcomeFacts {
        let s = &self.settings;
        let (minute, _) = clock_of(now);
        let day = day_of(now);
        // With plays off the view gets no history and no song band at all.
        let plays: Vec<_> = if s.plays {
            self.plays.in_window(0, 1440).into_iter().cloned().collect()
        } else {
            Vec::new()
        };
        let playing = s.plays && facts.media.playing && self.plays.current().is_some();
        let paused_last = paused_on_last(facts, &plays);
        WelcomeFacts {
            plays_on: s.plays,
            now: (day, minute),
            date: date_short(weekday(day), now.day, now.month, self.lang),
            plays,
            playing,
            forecast: self.forecast.clone(),
            weather_on: s.weather,
            span: s.span,
            unit: s.unit,
            head: s.head,
            sound: sound_head(facts, self.lang),
            paused_last,
            lang: self.lang,
        }
    }
}

/// The playing track as (title, artist); `None` when nothing plays.
fn playing_track(facts: &Facts) -> Option<(String, String)> {
    let media = &facts.media;
    (media.has_session && media.playing && !media.title.is_empty())
        .then(|| (media.title.clone(), media.artist.clone()))
}

/// The default output for the sound head slot; `None` while there is none.
fn sound_head(facts: &Facts, lang: Lang) -> Option<SoundHead> {
    let pill = &facts.pill;
    if pill.output.is_empty() {
        return None;
    }
    let kind = facts
        .audio
        .default_id
        .as_ref()
        .and_then(|id| facts.audio.endpoints.iter().find(|e| &e.id == id))
        .map(|e| device_kind(e.kind, lang).to_string())
        .unwrap_or_default();
    Some(SoundHead {
        output: pill.output.clone(),
        kind,
        value: pill.value,
        muted: pill.state == PillState::Muted,
        flowing: pill.state == PillState::Live,
    })
}

/// The default output in short for the resting pill (see `sound_words`); `None` while there is
/// no output or no volume to show.
fn pill_sound(facts: &Facts, lang: Lang) -> Option<SoundWords> {
    let s = sound_head(facts, lang)?;
    sound_words(s.value, s.muted, &s.kind, text(Text::Off, lang), lang)
}

/// True when a paused media session sits on the track of the last play: only then is that play
/// "paused" rather than just a past one.
fn paused_on_last(facts: &Facts, plays: &[plith_core::plays::Play]) -> bool {
    let media = &facts.media;
    media.has_session
        && !media.playing
        && !media.title.is_empty()
        && plays
            .last()
            .is_some_and(|p| p.title == media.title && p.artist == media.artist)
}

#[cfg(test)]
mod tests;
