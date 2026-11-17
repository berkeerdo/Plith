//! The idle look model: the settings, the play log and the forecast, turned into the facts the
//! resting pill and the welcome view are built from. Pure over its inputs; the feed decides
//! when to ask.

use crate::log::Stamp;
use crate::services::Facts;
use plith_core::lang::{Lang, date_short, device_kind, weekday};
use plith_core::pages::idle_pill::{IdleFacts, weather_words};
use plith_core::pages::pill::PillState;
use plith_core::pages::tabbar::clock_text;
use plith_core::pages::welcome::{SoundHead, WelcomeFacts};
use plith_core::plays::{Day, PlayLog};
use plith_core::settings::{IdleLook, OpenOn, Settings};
use plith_core::weather::{Forecast, Stamp as WeatherStamp};

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
        if !self.settings.plays {
            return;
        }
        let media = &facts.media;
        let track = (media.has_session && media.playing && !media.title.is_empty())
            .then(|| (media.title.clone(), media.artist.clone()));
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
        let media = &facts.media;
        // Allowed: an hour and a minute are below 60.
        #[allow(clippy::cast_possible_truncation)]
        let time = clock_text(now.hour as u8, now.minute as u8);
        IdleFacts {
            time,
            turn: words.turn,
            now_sky: words.now_sky,
            playing: (media.has_session && media.playing && !media.title.is_empty())
                .then(|| (media.title.clone(), media.artist.clone())),
            seal: facts.pill.clone(),
            lang: self.lang,
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
