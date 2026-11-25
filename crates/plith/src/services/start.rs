//! Starter threads: `start()` blocks (brightness up to 15 s, media about 8 s), so each service
//! starts on its own short-lived `plith-svc-<name>` thread. The UI thread subscribed and set the
//! waker before; the starter reports the status through a channel and posts one wake.

use plith_audio::endpoints::EndpointService;
use plith_audio::microphone::MicrophoneService;
use plith_audio::voicemeeter::VoicemeeterService;
use plith_host::log::LogFile;
use plith_media::MediaService;
use plith_shell::UiWaker;
use plith_system::BrightnessService;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

/// Which service a start report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceId {
    /// `VoicemeeterService`.
    Voicemeeter,
    /// `EndpointService`.
    Endpoints,
    /// `MicrophoneService`.
    Microphone,
    /// `MediaService`.
    Media,
    /// `BrightnessService`.
    Brightness,
}

impl ServiceId {
    /// Every service, in start order.
    pub const ALL: [ServiceId; 5] = [
        ServiceId::Voicemeeter,
        ServiceId::Endpoints,
        ServiceId::Microphone,
        ServiceId::Media,
        ServiceId::Brightness,
    ];

    /// The short name used in thread names and log lines.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ServiceId::Voicemeeter => "voicemeeter",
            ServiceId::Endpoints => "endpoints",
            ServiceId::Microphone => "microphone",
            ServiceId::Media => "media",
            ServiceId::Brightness => "brightness",
        }
    }
}

/// A finished `start()`.
#[derive(Debug, Clone)]
pub struct Started {
    /// The service.
    pub id: ServiceId,
    /// Its status, as text for the log.
    pub status: String,
}

/// Which starts have returned, readable from any thread (the idle check waits on it).
#[derive(Debug, Default)]
pub struct StartBoard {
    up: Mutex<Vec<ServiceId>>,
}

impl StartBoard {
    /// Records that the start of `id` returned; a repeat is ignored.
    pub fn mark(&self, id: ServiceId) {
        let mut up = self.up.lock().unwrap_or_else(PoisonError::into_inner);
        if !up.contains(&id) {
            up.push(id);
        }
    }

    /// The services whose start returned, in [`ServiceId::ALL`] order.
    #[must_use]
    pub fn up(&self) -> Vec<ServiceId> {
        let up = self.up.lock().unwrap_or_else(PoisonError::into_inner);
        ServiceId::ALL
            .into_iter()
            .filter(|id| up.contains(id))
            .collect()
    }

    /// True when every service's start returned.
    #[must_use]
    pub fn all_up(&self) -> bool {
        self.up().len() == ServiceId::ALL.len()
    }
}

/// The services to start.
pub struct Set {
    pub vm: Arc<VoicemeeterService>,
    pub endpoints: Arc<EndpointService>,
    pub mic: Arc<MicrophoneService>,
    pub media: Arc<MediaService>,
    pub brightness: Arc<BrightnessService>,
}

/// Sets every waker (before any `start()`), then starts each service on its own thread. The
/// returned receiver gets one [`Started`] per service, and `board` marks each finished start.
pub fn spawn_all(
    waker: &UiWaker,
    log: &LogFile,
    set: Set,
    board: &Arc<StartBoard>,
) -> Receiver<Started> {
    let wake = || -> Box<dyn Fn() + Send + Sync> {
        let waker = waker.clone();
        Box::new(move || waker.wake())
    };
    set.vm.set_waker(wake());
    set.endpoints.set_waker(wake());
    set.mic.set_waker(wake());
    set.media.set_waker(wake());
    set.brightness.set_waker(wake());
    let (tx, rx) = channel();
    let Set {
        vm,
        endpoints,
        mic,
        media,
        brightness,
    } = set;
    let starters: [(ServiceId, Box<dyn FnOnce() -> String + Send>); 5] = [
        (
            ServiceId::Voicemeeter,
            Box::new(move || format!("{:?}", vm.start())),
        ),
        (
            ServiceId::Endpoints,
            Box::new(move || format!("{:?}", endpoints.start())),
        ),
        (
            ServiceId::Microphone,
            Box::new(move || format!("{:?}", mic.start())),
        ),
        (
            ServiceId::Media,
            Box::new(move || format!("{:?}", media.start())),
        ),
        (
            ServiceId::Brightness,
            Box::new(move || format!("{:?}", brightness.start())),
        ),
    ];
    for (id, start) in starters {
        let report = Report {
            waker: waker.clone(),
            done: tx.clone(),
            board: Arc::clone(board),
        };
        if let Err(error) = spawn(id, start, report) {
            log.line(&format!(
                "service {}: no starter thread: {error}",
                id.name()
            ));
        }
    }
    rx
}

/// Where a starter reports a finished start.
struct Report {
    waker: UiWaker,
    done: Sender<Started>,
    board: Arc<StartBoard>,
}

/// Runs `start` on a new `plith-svc-<name>` thread, then reports, marks the board and wakes
/// the UI thread. The thread is detached: it ends right after `start()` returns, and at exit
/// nothing waits for a start that is still blocking.
fn spawn(
    id: ServiceId,
    start: Box<dyn FnOnce() -> String + Send>,
    report: Report,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name(format!("plith-svc-{}", id.name()))
        .spawn(move || {
            let status = start();
            let _ = report.done.send(Started { id, status });
            report.board.mark(id);
            report.waker.wake();
        })
        .map(drop)
}

#[cfg(test)]
mod tests;
