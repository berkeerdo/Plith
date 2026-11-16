//! The services the pages read: the real R2 services or the mock's fixtures, behind one
//! UI-thread trait. Nothing here blocks the UI thread: the real services start on their own
//! threads and only post a wake when they publish.

pub mod fixture;
mod start;

use crate::log::LogFile;
use crate::model;
use plith_audio::endpoints::{EndpointService, EndpointState};
use plith_audio::microphone::{MicSnapshot, MicrophoneService};
use plith_audio::voicemeeter::{VmSnapshot, VoicemeeterService};
use plith_core::draw::ArtPixels;
use plith_core::pages::audio::AudioFacts;
use plith_core::pages::derive;
use plith_core::pages::media::MediaFacts;
use plith_core::pages::pill::PillFacts;
use plith_core::pages::system::SystemFacts;
use plith_media::{MediaService, MediaSnapshot};
use plith_shell::UiWaker;
use plith_system::{BrightnessService, DeviceLevel};
use start::{ServiceId, Started};
use std::ops::{BitOr, BitOrAssign};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, SystemTime};

/// Which groups of facts changed since the last drain, as bit flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changes(u8);

impl Changes {
    /// Nothing changed.
    pub const NONE: Changes = Changes(0);
    /// Voicemeeter, the Windows endpoints or the microphone changed.
    pub const AUDIO: Changes = Changes(1);
    /// The media session changed.
    pub const MEDIA: Changes = Changes(2);
    /// The brightness changed.
    pub const SYSTEM: Changes = Changes(4);

    /// True when any flag is set.
    #[must_use]
    pub fn any(self) -> bool {
        self.0 != 0
    }

    /// True when any flag of `other` is set here too.
    #[must_use]
    pub fn has(self, other: Changes) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for Changes {
    type Output = Changes;
    fn bitor(self, other: Changes) -> Changes {
        Changes(self.0 | other.0)
    }
}

impl BitOrAssign for Changes {
    fn bitor_assign(&mut self, other: Changes) {
        self.0 |= other.0;
    }
}

/// Everything the pages are built from.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    /// The audio page.
    pub audio: AudioFacts,
    /// The media page.
    pub media: MediaFacts,
    /// The system page.
    pub system: SystemFacts,
    /// The collapsed pill.
    pub pill: PillFacts,
    /// The decoded cover art the media page shows, if any.
    pub art: Option<Arc<ArtPixels>>,
}

/// Peak levels (linear, 0 to 1) of every strip and bus, copied from Voicemeeter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Levels {
    /// Left and right peak per strip.
    pub strips: Vec<[f32; 2]>,
    /// Left and right peak per bus.
    pub buses: Vec<[f32; 2]>,
}

/// Reads levels on the render thread; `None` when there is no mixer.
pub type LevelSource = Box<dyn FnMut() -> Option<Levels> + Send>;

/// The UI thread's view of the services. Not `Send`: it lives in the app on the UI thread.
pub trait Services {
    /// Takes every published reading (the latest of each wins) and says what changed.
    fn drain(&mut self) -> Changes;
    /// The facts for the pages at `now_s` seconds of app time.
    fn facts(&self, now_s: f64) -> Facts;
    /// The current levels, for the signal activity; `None` without a mixer.
    fn levels(&self) -> Option<Levels>;
    /// A level reader for the render thread.
    fn level_source(&self) -> LevelSource;
    /// Makes the endpoint with this id the default output.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn set_default_endpoint(&mut self, id: &str);
    /// Flips the route from strip `strip` to bus `bus`.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn toggle_route(&mut self, strip: usize, bus: usize);
    /// Mutes or unmutes the microphone.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn toggle_mic(&mut self);
    /// Sets the monitor brightness in percent.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn set_brightness(&mut self, percent: u8);
    /// Toggles play and pause.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn play_pause(&mut self);
    /// Skips to the next track.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn next(&mut self);
    /// Skips to the previous track.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn previous(&mut self);
    /// Seeks to `position` in the track.
    #[allow(dead_code, reason = "Task 17 routes clicks to the writes")]
    fn seek(&mut self, position: Duration);
}

/// The latest reading of every service.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Readings {
    /// The mixer; `None` while Voicemeeter is absent or starting.
    pub vm: Option<VmSnapshot>,
    /// The Windows render endpoints.
    pub endpoints: EndpointState,
    /// The default microphone.
    pub mic: Option<MicSnapshot>,
    /// The media session.
    pub media: MediaSnapshot,
    /// Every controllable display, built-in first.
    pub brightness: Vec<DeviceLevel>,
}

/// The receivers subscribed before each service started.
struct Feeds {
    vm: Receiver<Option<VmSnapshot>>,
    endpoints: Receiver<EndpointState>,
    mic: Receiver<Option<MicSnapshot>>,
    media: Receiver<MediaSnapshot>,
    brightness: Receiver<Vec<DeviceLevel>>,
}

/// The R2 services, each started on its own `plith-svc-<name>` thread.
pub struct RealServices {
    vm: Arc<VoicemeeterService>,
    endpoints: Arc<EndpointService>,
    mic: Arc<MicrophoneService>,
    media: Arc<MediaService>,
    brightness: Arc<BrightnessService>,
    feeds: Feeds,
    started: Receiver<Started>,
    readings: Readings,
    log: Arc<LogFile>,
}

/// The newest value waiting on `rx`, if any.
fn latest<T>(rx: &Receiver<T>) -> Option<T> {
    rx.try_iter().last()
}

/// Stores `next` and returns `flag` when it differs from `slot`.
fn take<T: PartialEq>(slot: &mut T, next: Option<T>, flag: Changes) -> Changes {
    match next {
        Some(value) if *slot != value => {
            *slot = value;
            flag
        }
        _ => Changes::NONE,
    }
}

impl RealServices {
    /// Subscribes to every service, sets its waker to `waker` and starts it on its own thread.
    /// Returns at once; the pages show degraded states until the services publish.
    #[must_use]
    pub fn start(waker: &UiWaker, log: Arc<LogFile>) -> Self {
        let (vm, endpoints, mic) = (
            Arc::new(VoicemeeterService::new()),
            Arc::new(EndpointService::new()),
            Arc::new(MicrophoneService::new()),
        );
        let (media, brightness) = (
            Arc::new(MediaService::new()),
            Arc::new(BrightnessService::new()),
        );
        let feeds = Feeds {
            vm: vm.subscribe(),
            endpoints: endpoints.subscribe(),
            mic: mic.subscribe(),
            media: media.subscribe(),
            brightness: brightness.subscribe(),
        };
        let started = start::spawn_all(
            waker,
            &log,
            start::Set {
                vm: Arc::clone(&vm),
                endpoints: Arc::clone(&endpoints),
                mic: Arc::clone(&mic),
                media: Arc::clone(&media),
                brightness: Arc::clone(&brightness),
            },
        );
        Self {
            vm,
            endpoints,
            mic,
            media,
            brightness,
            feeds,
            started,
            readings: Readings::default(),
            log,
        }
    }

    /// Logs a finished start and re-reads that service's snapshot (a reading published before
    /// the subscription would otherwise be missed).
    fn on_started(&mut self, started: &Started) -> Changes {
        self.log.line(&format!(
            "service {}: {}",
            started.id.name(),
            started.status
        ));
        let r = &mut self.readings;
        match started.id {
            ServiceId::Voicemeeter => take(&mut r.vm, Some(self.vm.snapshot()), Changes::AUDIO),
            ServiceId::Endpoints => take(
                &mut r.endpoints,
                Some(self.endpoints.snapshot()),
                Changes::AUDIO,
            ),
            ServiceId::Microphone => take(&mut r.mic, Some(self.mic.snapshot()), Changes::AUDIO),
            ServiceId::Media => take(&mut r.media, Some(self.media.snapshot()), Changes::MEDIA),
            ServiceId::Brightness => take(
                &mut r.brightness,
                Some(self.brightness.snapshot()),
                Changes::SYSTEM,
            ),
        }
    }
}

impl Services for RealServices {
    fn drain(&mut self) -> Changes {
        let mut changes = Changes::NONE;
        while let Ok(started) = self.started.try_recv() {
            changes |= self.on_started(&started);
        }
        let (r, f) = (&mut self.readings, &self.feeds);
        changes |= take(&mut r.vm, latest(&f.vm), Changes::AUDIO);
        changes |= take(&mut r.endpoints, latest(&f.endpoints), Changes::AUDIO);
        changes |= take(&mut r.mic, latest(&f.mic), Changes::AUDIO);
        changes |= take(&mut r.media, latest(&f.media), Changes::MEDIA);
        changes |= take(&mut r.brightness, latest(&f.brightness), Changes::SYSTEM);
        changes
    }

    fn facts(&self, _now_s: f64) -> Facts {
        let signal = self
            .levels()
            .is_some_and(|levels| derive::any_signal(&levels.strips));
        model::facts(&self.readings, signal, SystemTime::now())
    }

    fn levels(&self) -> Option<Levels> {
        self.readings.vm.as_ref()?;
        let levels = self.vm.levels();
        Some(Levels {
            strips: levels.strips,
            buses: levels.buses,
        })
    }

    fn level_source(&self) -> LevelSource {
        let vm = Arc::clone(&self.vm);
        Box::new(move || {
            let levels = vm.levels();
            (!levels.strips.is_empty()).then_some(Levels {
                strips: levels.strips,
                buses: levels.buses,
            })
        })
    }

    fn set_default_endpoint(&mut self, id: &str) {
        let _ = self.endpoints.set_default_endpoint(id);
    }

    fn toggle_route(&mut self, strip: usize, bus: usize) {
        let _ = self.vm.toggle_route(strip, bus);
    }

    fn toggle_mic(&mut self) {
        let _ = self.mic.toggle_mute();
    }

    fn set_brightness(&mut self, percent: u8) {
        let _ = self.brightness.set_percent(percent);
    }

    fn play_pause(&mut self) {
        let _ = self.media.toggle_play_pause();
    }

    fn next(&mut self) {
        let _ = self.media.next();
    }

    fn previous(&mut self) {
        let _ = self.media.previous();
    }

    fn seek(&mut self, position: Duration) {
        let _ = self.media.seek(position);
    }
}
