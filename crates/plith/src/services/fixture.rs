//! `--fixture`: the mock's data instead of the hardware. Its writes change the fixture only,
//! and its levels come from a deterministic generator.

use super::{Changes, Facts, LevelSource, Levels, Services};
use plith_core::draw::{ArtPixels, SeekTimeline};
use plith_core::launch::Fixture;
use plith_core::pages::audio::{AudioFacts, MicFacts};
use plith_core::pages::derive;
use plith_core::pages::fixtures;
use plith_core::pages::media::MediaFacts;
use plith_core::pages::system::{self, SystemFacts};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime};

/// The fixture microphone.
const MIC_NAME: &str = "SHURE MV7";

/// Levels at `t` seconds: a slow sine per strip and bus around a third of full scale, silent
/// on a muted strip. Deterministic, so tests can pin it.
#[must_use]
pub fn fixture_levels(muted: &[bool], buses: usize, t: f64) -> Levels {
    let wave = |i: usize, base: f64, swing: f64| {
        // Allowed: indices are tiny, exact in f64.
        #[allow(clippy::cast_precision_loss)]
        let phase = t * 2.3 + i as f64 * 1.1;
        let side = |p: f64| {
            // Allowed: the value stays within 0.05..0.6.
            #[allow(clippy::cast_possible_truncation)]
            let level = (base + swing * p.sin()) as f32;
            level
        };
        [side(phase), side(phase + 0.6)]
    };
    Levels {
        strips: muted
            .iter()
            .enumerate()
            .map(|(i, m)| if *m { [0.0, 0.0] } else { wave(i, 0.3, 0.25) })
            .collect(),
        buses: (0..buses).map(|i| wave(i + 8, 0.35, 0.2)).collect(),
    }
}

/// What a content variant swaps: everything the fixture shows besides the levels.
#[derive(Clone)]
struct Content {
    audio: AudioFacts,
    media: MediaFacts,
    brightness: Option<(String, u8)>,
    art: Arc<ArtPixels>,
}

/// The mock's data as a service.
pub struct FixtureServices {
    launched: Content,
    variants: Option<Receiver<u8>>,
    shown: u8,
    audio: AudioFacts,
    media: MediaFacts,
    brightness: Option<(String, u8)>,
    art: Arc<ArtPixels>,
    pending: Changes,
    origin: Instant,
    wake: Option<Box<dyn Fn()>>,
}

impl FixtureServices {
    /// The fixture `kind`; the media timeline starts now.
    #[must_use]
    pub fn new(kind: Fixture) -> Self {
        let windows = fixtures::no_voicemeeter();
        let vm = match kind {
            Fixture::Banana => fixtures::banana().vm,
            Fixture::Potato => fixtures::potato().vm,
            Fixture::Bare => None,
        };
        let audio = AudioFacts {
            vm,
            mic: Some(MicFacts {
                name: MIC_NAME.to_string(),
                muted: false,
            }),
            ..windows
        };
        let mut media = fixtures::media_playing();
        if let Some(timeline) = media.timeline.as_mut() {
            timeline.stamp = SystemTime::now();
        }
        let brightness = fixtures::system().brightness;
        let art = Arc::new(fixtures::art());
        Self {
            launched: Content {
                audio: audio.clone(),
                media: media.clone(),
                brightness: brightness.clone(),
                art: Arc::clone(&art),
            },
            variants: None,
            shown: 0,
            audio,
            media,
            brightness,
            art,
            pending: Changes::NONE,
            origin: Instant::now(),
            wake: None,
        }
    }

    /// Calls `wake` after every write that changed something.
    #[must_use]
    pub fn with_waker(self, wake: Box<dyn Fn()>) -> Self {
        Self {
            wake: Some(wake),
            ..self
        }
    }

    /// Takes content variant requests from `variants` on every drain (see [`Self::switch`]).
    #[must_use]
    pub fn with_content(self, variants: Receiver<u8>) -> Self {
        Self {
            variants: Some(variants),
            ..self
        }
    }

    /// The content of variant `n`: 0 the fixture as launched, 1 the next track with new art,
    /// 2 a route toggled and one source muted, 3 the brightness changed. `None` for any other.
    fn variant(&self, n: u8) -> Option<Content> {
        let mut content = self.launched.clone();
        match n {
            0 => {}
            1 => {
                let mut next = fixtures::media_next();
                next.output.clone_from(&content.media.output);
                if let Some(timeline) = next.timeline.as_mut() {
                    timeline.stamp = SystemTime::now();
                }
                content.media = next;
                content.art = Arc::new(fixtures::art_next());
            }
            2 => {
                let vm = content.audio.vm.as_mut()?;
                let strip = vm.strips.get_mut(2)?;
                strip.routes.resize(vm.buses.len(), false);
                let bus = strip.routes.len().checked_sub(1)?;
                strip.routes[bus] = !strip.routes[bus];
                vm.strips.get_mut(3)?.muted = true;
            }
            3 => content.brightness.as_mut()?.1 = 35,
            _ => return None,
        }
        Some(content)
    }

    /// Switches to variant `n` and marks what differs; an unknown or already shown variant
    /// changes nothing.
    fn switch(&mut self, n: u8) {
        if n == self.shown {
            return;
        }
        let Some(next) = self.variant(n) else { return };
        self.shown = n;
        let mut changes = Changes::NONE;
        if next.audio != self.audio {
            changes |= Changes::AUDIO;
        }
        if next.media != self.media || next.art.id != self.art.id {
            changes |= Changes::MEDIA;
        }
        if next.brightness != self.brightness {
            changes |= Changes::SYSTEM;
        }
        (self.audio, self.media) = (next.audio, next.media);
        (self.brightness, self.art) = (next.brightness, next.art);
        if changes.any() {
            self.mark(changes);
        }
    }

    /// Records a change and wakes the UI thread, as a real service would.
    fn mark(&mut self, changes: Changes) {
        self.pending |= changes;
        if let Some(wake) = &self.wake {
            wake();
        }
    }

    /// Seconds since the fixture started: the one time origin of its levels and facts.
    fn now_s(&self) -> f64 {
        self.origin.elapsed().as_secs_f64()
    }

    /// The muted flag of every strip.
    fn muted(&self) -> Vec<bool> {
        self.audio
            .vm
            .as_ref()
            .map(|vm| vm.strips.iter().map(|s| s.muted).collect())
            .unwrap_or_default()
    }

    /// The number of buses, 0 without a mixer.
    fn buses(&self) -> usize {
        self.audio.vm.as_ref().map_or(0, |vm| vm.buses.len())
    }

    /// The levels at `t` seconds; `None` without a mixer.
    fn levels_at(&self, t: f64) -> Option<Levels> {
        self.audio.vm.as_ref()?;
        Some(fixture_levels(&self.muted(), self.buses(), t))
    }

    /// Changes the media timeline as of now and marks the media changed.
    fn move_media(&mut self, change: impl FnOnce(&mut MediaFacts, SeekTimeline) -> SeekTimeline) {
        if let Some(timeline) = self.media.timeline {
            let current = derive::rebase(timeline, SystemTime::now());
            self.media.timeline = Some(change(&mut self.media, current));
            self.mark(Changes::MEDIA);
        }
    }
}

impl Services for FixtureServices {
    fn drain(&mut self) -> Changes {
        let asked: Vec<u8> = self.variants.iter().flat_map(Receiver::try_iter).collect();
        for n in asked {
            self.switch(n);
        }
        std::mem::take(&mut self.pending)
    }

    /// The caller's `now_s` is ignored: the levels the render thread draws, `levels()` and the
    /// pill all read the fixture's own clock.
    fn facts(&self, _now_s: f64) -> Facts {
        let signal = self
            .levels_at(self.now_s())
            .is_some_and(|levels| derive::any_signal(&levels.strips));
        let pill = derive::pill_facts(&self.audio, self.media.playing, signal);
        let mut media = self.media.clone();
        media.timeline = media.timeline.map(|t| derive::rebase(t, SystemTime::now()));
        media.output = (!pill.output.is_empty()).then(|| pill.output.clone());
        let system = SystemFacts {
            brightness: self.brightness.clone(),
            mic: self.audio.mic.as_ref().map(|m| system::MicFacts {
                name: derive::mic_caption(&m.name, self.audio.vm.is_some()),
                muted: m.muted,
            }),
            mic_bus: self.audio.vm.as_ref().and_then(derive::mic_bus),
        };
        Facts {
            audio: self.audio.clone(),
            art: media.art.map(|_| Arc::clone(&self.art)),
            media,
            system,
            pill,
        }
    }

    fn levels(&self) -> Option<Levels> {
        self.levels_at(self.now_s())
    }

    fn level_source(&self) -> LevelSource {
        let (muted, buses, origin) = (self.muted(), self.buses(), self.origin);
        let mixer = self.audio.vm.is_some();
        Box::new(move || {
            mixer.then(|| fixture_levels(&muted, buses, origin.elapsed().as_secs_f64()))
        })
    }

    fn set_default_endpoint(&mut self, id: &str) {
        let known = self.audio.endpoints.iter().any(|e| e.id == id);
        if known && self.audio.default_id.as_deref() != Some(id) {
            self.audio.default_id = Some(id.to_string());
            self.mark(Changes::AUDIO);
        }
    }

    fn toggle_route(&mut self, strip: usize, bus: usize) {
        let Some(vm) = self.audio.vm.as_mut() else {
            return;
        };
        let buses = vm.buses.len();
        let Some(strip) = vm.strips.get_mut(strip) else {
            return;
        };
        if bus >= buses {
            return;
        }
        strip.routes.resize(buses, false);
        strip.routes[bus] = !strip.routes[bus];
        self.mark(Changes::AUDIO);
    }

    fn toggle_mic(&mut self) {
        if let Some(mic) = self.audio.mic.as_mut() {
            mic.muted = !mic.muted;
            self.mark(Changes::AUDIO);
        }
    }

    fn set_brightness(&mut self, percent: u8) {
        if let Some((_, level)) = self.brightness.as_mut() {
            *level = percent.min(100);
            self.mark(Changes::SYSTEM);
        }
    }

    fn play_pause(&mut self) {
        self.move_media(|media, timeline| {
            media.playing = !media.playing;
            SeekTimeline {
                playing: media.playing,
                ..timeline
            }
        });
    }

    fn next(&mut self) {
        self.move_media(|_, timeline| SeekTimeline {
            position_s: 0.0,
            ..timeline
        });
    }

    fn previous(&mut self) {
        self.move_media(|_, timeline| SeekTimeline {
            position_s: 0.0,
            ..timeline
        });
    }

    fn seek(&mut self, position: Duration) {
        self.move_media(|_, timeline| SeekTimeline {
            position_s: position.as_secs_f64().min(timeline.duration_s),
            ..timeline
        });
    }
}

#[cfg(test)]
mod tests;
