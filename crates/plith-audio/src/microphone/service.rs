//! `MicrophoneService`: one thread owns the capture attachment, like `EndpointService`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use windows::Win32::Media::Audio::{
    IMMDeviceEnumerator, IMMNotificationClient, eCapture, eCommunications,
};

use super::MicSnapshot;
use crate::endpoints::Status;
use crate::endpoints::com::{Attachment, ComGuard, create_enumerator, default_endpoint};
use crate::endpoints::notify::{DeviceEvents, Msg, ask};

type Snapshot = Option<MicSnapshot>;

/// State shared between the handle and the service thread; locks are plain reads and writes.
struct Shared {
    stop: AtomicBool,
    snapshot: Mutex<Snapshot>,
    subscribers: Mutex<Vec<Sender<Snapshot>>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn publish(&self, next: &Snapshot) {
        {
            let mut current = locked(&self.snapshot);
            if *current == *next {
                return;
            }
            current.clone_from(next);
        }
        locked(&self.subscribers).retain(|tx| tx.send(next.clone()).is_ok());
    }
}

struct Worker {
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    en: IMMDeviceEnumerator,
    attachment: Option<(Attachment, String)>,
    generation: u64,
}

impl Worker {
    /// Re-resolves the default capture device, re-attaches when it changed, then publishes.
    fn refresh(&mut self) {
        let target = default_endpoint(&self.en, eCapture, eCommunications);
        let same = match (&self.attachment, &target) {
            (Some((a, _)), Some(t)) => a.id.eq_ignore_ascii_case(&t.id),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.detach();
            if let Some(endpoint) = &target {
                self.generation += 1;
                if let Ok((attachment, _)) =
                    Attachment::attach(&self.en, &endpoint.id, self.generation, &self.tx)
                {
                    self.attachment = Some((attachment, endpoint.friendly_name.clone()));
                }
            }
        }
        self.publish_current();
    }

    fn publish_current(&mut self) {
        let snapshot = self
            .attachment
            .as_ref()
            .and_then(|(a, name)| MicSnapshot::from_state(name, a.read_state()));
        if snapshot.is_none() {
            self.detach();
        }
        self.shared.publish(&snapshot);
    }

    fn detach(&mut self) {
        if let Some((attachment, _)) = self.attachment.take() {
            attachment.detach();
        }
    }

    fn on_volume(&self, generation: u64, volume: f32, muted: bool) {
        if self.attachment.as_ref().map(|(a, _)| a.generation) != Some(generation) {
            return;
        }
        let mut next = locked(&self.shared.snapshot).clone();
        if let Some(snapshot) = next.as_mut() {
            snapshot.volume = volume;
            snapshot.muted = muted;
            self.shared.publish(&next);
        }
    }
}

fn run(
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    rx: &Receiver<Msg>,
    ready: &SyncSender<Result<(), String>>,
) {
    let _com = match ComGuard::init() {
        Ok(guard) => guard,
        Err(err) => {
            let _ = ready.send(Err(format!("COM initialization failed: {err}")));
            return;
        }
    };
    let en = match create_enumerator() {
        Ok(en) => en,
        Err(err) => {
            let _ = ready.send(Err(format!("no audio device enumerator: {err}")));
            return;
        }
    };
    let client: IMMNotificationClient = DeviceEvents {
        tx: tx.clone(),
        flow: eCapture,
        role: eCommunications,
    }
    .into();
    // SAFETY: `client` lives until it is unregistered below.
    if let Err(err) = unsafe { en.RegisterEndpointNotificationCallback(&client) } {
        let _ = ready.send(Err(format!("could not register for device events: {err}")));
        return;
    }
    let mut worker = Worker {
        shared,
        tx,
        en,
        attachment: None,
        generation: 0,
    };
    worker.refresh();
    let _ = ready.send(Ok(()));
    serve(&mut worker, rx);

    // Shutdown on this thread with no lock held, as in the endpoints service.
    worker.detach();
    // SAFETY: `client` is the callback registered above.
    let _ = unsafe { worker.en.UnregisterEndpointNotificationCallback(&client) };
}

fn serve(worker: &mut Worker, rx: &Receiver<Msg>) {
    while let Ok(msg) = rx.recv() {
        if worker.shared.stop.load(Ordering::Acquire) {
            break;
        }
        match msg {
            Msg::Refresh => worker.refresh(),
            Msg::Volume {
                generation,
                volume,
                muted,
            } => worker.on_volume(generation, volume, muted),
            Msg::ToggleMute { reply } => {
                let _ = reply.send(
                    worker
                        .attachment
                        .as_ref()
                        .and_then(|(a, _)| a.toggle_mute()),
                );
            }
            // Volume and default-device writes are not part of the microphone's surface.
            Msg::SetVolume { reply, .. } | Msg::SetDefault { reply, .. } => {
                let _ = reply.send(false);
            }
            Msg::Stop => break,
        }
    }
}

/// Headless default-capture-device service.
///
/// Create with [`MicrophoneService::new`], then [`start`](Self::start). With no capture device
/// the snapshot is `None`; nothing panics.
pub struct MicrophoneService {
    shared: Arc<Shared>,
    control: Mutex<Option<Sender<Msg>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Default for MicrophoneService {
    fn default() -> Self {
        Self::new()
    }
}

impl MicrophoneService {
    /// Creates an idle service; nothing touches COM until [`start`](Self::start).
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                stop: AtomicBool::new(false),
                snapshot: Mutex::new(None),
                subscribers: Mutex::new(Vec::new()),
            }),
            control: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts the service thread and waits for the first reading. Calling it while running
    /// returns [`Status::Running`].
    pub fn start(&self) -> Status {
        let mut slot = locked(&self.thread);
        if slot.is_some() {
            return Status::Running;
        }
        self.shared.stop.store(false, Ordering::Release);
        let (tx, rx) = channel();
        let (ready_tx, ready_rx) = sync_channel(1);
        let (shared, worker_tx) = (Arc::clone(&self.shared), tx.clone());
        let spawned = thread::Builder::new()
            .name("plith-microphone".into())
            .spawn(move || run(shared, worker_tx, &rx, &ready_tx));
        let handle = match spawned {
            Ok(handle) => handle,
            Err(err) => return Status::Unavailable(format!("could not start the thread: {err}")),
        };
        match ready_rx.recv() {
            Ok(Ok(())) => {
                *locked(&self.control) = Some(tx);
                *slot = Some(handle);
                Status::Running
            }
            Ok(Err(reason)) => {
                let _ = handle.join();
                Status::Unavailable(reason)
            }
            Err(_) => {
                let _ = handle.join();
                Status::Unavailable("the service thread ended before it was ready".into())
            }
        }
    }

    /// Stops the thread, which unregisters its callbacks itself, and waits for it. Safe to
    /// call twice.
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(tx) = locked(&self.control).take() {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(handle) = locked(&self.thread).take() {
            let _ = handle.join();
        }
        self.shared.publish(&None);
    }

    /// Latest reading; `None` means no capture device (or not started) and must not be shown
    /// as unmuted.
    #[must_use]
    pub fn snapshot(&self) -> Option<MicSnapshot> {
        locked(&self.shared.snapshot).clone()
    }

    /// A channel of readings, one per change. Drain it to the latest value.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Option<MicSnapshot>> {
        let (tx, rx) = channel();
        locked(&self.shared.subscribers).push(tx);
        rx
    }

    /// Reads the microphone's mute and writes the opposite. Returns the new state, or `None`
    /// when there is no capture device or the write failed.
    pub fn toggle_mute(&self) -> Option<bool> {
        let tx = locked(&self.control).clone()?;
        ask(&tx, |reply| Msg::ToggleMute { reply }).flatten()
    }
}

impl Drop for MicrophoneService {
    fn drop(&mut self) {
        self.stop();
    }
}
