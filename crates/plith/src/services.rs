//! The services the pages read: the real R2 services or the mock's fixtures, behind one
//! UI-thread trait. Nothing here blocks the UI thread: the real services start on their own
//! threads and only post a wake when they publish.

pub mod fixture;
pub mod start;
pub mod writes;

use crate::log::LogFile;
use crate::model;
use plith_audio::endpoints::{EndpointService, EndpointState};
use plith_audio::microphone::{MicSnapshot, MicrophoneService};
use plith_audio::voicemeeter::{VmSnapshot, VoicemeeterService};
use plith_core::draw::ArtPixels;
use plith_core::pages::audio::AudioFacts;
use plith_core::pages::media::MediaFacts;
use plith_core::pages::pill::PillFacts;
use plith_core::pages::system::SystemFacts;
use plith_media::{MediaService, MediaSnapshot};
use plith_shell::UiWaker;
use plith_system::{BrightnessService, DeviceLevel};
use start::{ServiceId, StartBoard, Started};
use std::ops::{BitOr, BitOrAssign};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime};
use writes::{Write, Writer};

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
    /// The facts for the pages. A pure read: the pill's signal comes debounced from the mixer
    /// service, so nothing here observes levels.
    fn facts(&self) -> Facts;
    /// The current levels, for the signal activity; `None` without a mixer.
    fn levels(&self) -> Option<Levels>;
    /// A level reader for the render thread.
    fn level_source(&self) -> LevelSource;
    /// Makes the endpoint with this id the default output.
    fn set_default_endpoint(&mut self, id: &str);
    /// Flips the route from strip `strip` to bus `bus`.
    fn toggle_route(&mut self, strip: usize, bus: usize);
    /// Mutes or unmutes the microphone.
    fn toggle_mic(&mut self);
    /// Sets the monitor brightness in percent.
    fn set_brightness(&mut self, percent: u8);
    /// Toggles play and pause.
    fn play_pause(&mut self);
    /// Skips to the next track.
    fn next(&mut self);
    /// Skips to the previous track.
    fn previous(&mut self);
    /// Seeks to `position` in the track.
    fn seek(&mut self, position: Duration);
    /// Stops the services at shutdown; nothing to stop by default.
    fn stop(&mut self) {}
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
    /// The services whose `start()` returned.
    up: Vec<ServiceId>,
    /// Applies the writes off the UI thread; `None` when its thread did not start.
    writer: Option<Writer>,
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
    /// Subscribes to every service, sets its waker to `waker` and starts it on its own thread;
    /// `board` marks each finished start. Returns at once; the pages show degraded states until
    /// the services publish.
    #[must_use]
    pub fn start(waker: &UiWaker, log: Arc<LogFile>, board: &Arc<StartBoard>) -> Self {
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
            board,
        );
        let writer = spawn_writer(
            start::Set {
                vm: Arc::clone(&vm),
                endpoints: Arc::clone(&endpoints),
                mic: Arc::clone(&mic),
                media: Arc::clone(&media),
                brightness: Arc::clone(&brightness),
            },
            &log,
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
            up: Vec::new(),
            writer,
        }
    }

    /// Queues a write for the writes thread.
    fn write(&self, write: Write) {
        if let Some(writer) = &self.writer {
            writer.send(write);
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
        self.up.push(started.id);
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

    fn facts(&self) -> Facts {
        model::facts(&self.readings, SystemTime::now())
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
        self.write(Write::DefaultEndpoint(id.to_string()));
    }

    fn toggle_route(&mut self, strip: usize, bus: usize) {
        self.write(Write::Route { strip, bus });
    }

    fn toggle_mic(&mut self) {
        self.write(Write::ToggleMic);
    }

    fn set_brightness(&mut self, percent: u8) {
        self.write(Write::Brightness(percent));
    }

    fn play_pause(&mut self) {
        self.write(Write::PlayPause);
    }

    fn next(&mut self) {
        self.write(Write::Next);
    }

    fn previous(&mut self) {
        self.write(Write::Previous);
    }

    fn seek(&mut self, position: Duration) {
        self.write(Write::Seek(position));
    }

    /// Stops every service whose start returned, logging how long each stop took. A service
    /// still inside `start()` is left to its starter thread (its `stop` would wait for the
    /// start, up to 15 s for brightness).
    fn stop(&mut self) {
        if let Some(writer) = &mut self.writer {
            writer.stop();
        }
        while let Ok(started) = self.started.try_recv() {
            let _ = self.on_started(&started);
        }
        for id in ServiceId::ALL {
            if !self.up.contains(&id) {
                self.log.line(&format!(
                    "service {}: still starting, not stopped",
                    id.name()
                ));
                continue;
            }
            let begun = Instant::now();
            match id {
                ServiceId::Voicemeeter => self.vm.stop(),
                ServiceId::Endpoints => self.endpoints.stop(),
                ServiceId::Microphone => self.mic.stop(),
                ServiceId::Media => self.media.stop(),
                ServiceId::Brightness => self.brightness.stop(),
            }
            self.log.line(&format!(
                "service {}: stopped in {:.1} ms",
                id.name(),
                begun.elapsed().as_secs_f64() * 1000.0
            ));
        }
    }
}

/// Starts the writes thread over the services in `set`. Each write is one service call; its
/// error is dropped, as a failed write simply publishes no change.
fn spawn_writer(set: start::Set, log: &LogFile) -> Option<Writer> {
    let spawned = Writer::spawn(move |write| match write {
        Write::DefaultEndpoint(id) => {
            let _ = set.endpoints.set_default_endpoint(&id);
        }
        Write::Route { strip, bus } => {
            let _ = set.vm.toggle_route(strip, bus);
        }
        Write::ToggleMic => {
            let _ = set.mic.toggle_mute();
        }
        Write::Brightness(percent) => {
            let _ = set.brightness.set_percent(percent);
        }
        Write::PlayPause => {
            let _ = set.media.toggle_play_pause();
        }
        Write::Next => {
            let _ = set.media.next();
        }
        Write::Previous => {
            let _ = set.media.previous();
        }
        Write::Seek(position) => {
            let _ = set.media.seek(position);
        }
    });
    match spawned {
        Ok(writer) => Some(writer),
        Err(error) => {
            log.line(&format!("writes thread not started: {error}"));
            None
        }
    }
}
