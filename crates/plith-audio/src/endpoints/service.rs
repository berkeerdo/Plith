//! `EndpointService`: one thread owns every COM attach and detach; the handle only reads
//! shared state and posts messages.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use windows::Win32::Media::Audio::{IMMDeviceEnumerator, IMMNotificationClient};

use super::com::{Attachment, ComGuard, create_enumerator, default_id, list_active};
use super::notify::{DeviceEvents, Msg};
use super::{AttachedState, EndpointState, resolve_target};

/// Whether the Core Audio stack could be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The service thread is running and registered for notifications.
    Running,
    /// Not usable; the text says why. The snapshot stays empty.
    Unavailable(String),
}

/// State shared between the handle and the service thread. Every lock is held only for a
/// plain read or write, never across a COM call.
struct Shared {
    stop: AtomicBool,
    state: Mutex<EndpointState>,
    pin: Mutex<Option<String>>,
    subscribers: Mutex<Vec<Sender<EndpointState>>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            state: Mutex::new(EndpointState::default()),
            pin: Mutex::new(None),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Stores `next` and, when it differs from the current state, sends it to subscribers.
    fn publish(&self, next: &EndpointState) {
        {
            let mut current = locked(&self.state);
            if *current == *next {
                return;
            }
            current.clone_from(next);
        }
        locked(&self.subscribers).retain(|tx| tx.send(next.clone()).is_ok());
    }

    fn current(&self) -> EndpointState {
        locked(&self.state).clone()
    }
}

/// What the service thread owns besides the COM objects.
struct Worker {
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    en: IMMDeviceEnumerator,
    attachment: Option<Attachment>,
    generation: u64,
}

impl Worker {
    /// Re-reads the list and the default, re-resolves the target and re-attaches when it
    /// changed, then publishes. Runs only on the service thread.
    fn refresh(&mut self) {
        let active = list_active(&self.en);
        let default = default_id(&self.en);
        let pin = locked(&self.shared.pin).clone();
        let target = resolve_target(pin.as_deref(), &active, default.as_deref());

        let same = match (&self.attachment, &target) {
            (Some(a), Some(t)) => a.id.eq_ignore_ascii_case(t),
            (None, None) => true,
            _ => false,
        };
        let attached = if same {
            self.reread()
        } else {
            self.detach();
            target.and_then(|id| self.attach(&id))
        };
        self.shared.publish(&EndpointState {
            default_id: default,
            active,
            attached,
        });
    }

    /// Current state of the existing attachment; drops it when the endpoint stopped answering.
    fn reread(&mut self) -> Option<AttachedState> {
        let state = self.attachment.as_ref()?.read_state();
        if state.is_none() {
            self.detach();
        }
        state
    }

    fn attach(&mut self, id: &str) -> Option<AttachedState> {
        self.generation += 1;
        let (attachment, state) =
            Attachment::attach(&self.en, id, self.generation, &self.tx).ok()?;
        self.attachment = Some(attachment);
        Some(state)
    }

    fn detach(&mut self) {
        if let Some(attachment) = self.attachment.take() {
            attachment.detach();
        }
    }

    fn on_volume(&self, generation: u64, volume: f32, muted: bool) {
        if self.attachment.as_ref().map(|a| a.generation) != Some(generation) {
            return;
        }
        let mut next = self.shared.current();
        if let Some(attached) = next.attached.as_mut() {
            attached.volume = volume;
            attached.muted = muted;
            self.shared.publish(&next);
        }
    }
}

/// The service thread body. `ready` gets exactly one message: `Ok` once registered and the
/// first state is published, or the reason it cannot run.
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
    let client: IMMNotificationClient = DeviceEvents { tx: tx.clone() }.into();
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

    // Shutdown, on this thread and with no lock held: callbacks only post to the channel, so
    // the unregister calls cannot wait on anything we hold.
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
            Msg::Stop => break,
        }
    }
}

/// Headless Core Audio render endpoint service (read side).
///
/// Create with [`EndpointService::new`], then [`start`](Self::start). Without an audio stack
/// or without a default endpoint the state stays empty; nothing panics.
pub struct EndpointService {
    shared: Arc<Shared>,
    control: Mutex<Option<Sender<Msg>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Default for EndpointService {
    fn default() -> Self {
        Self::new()
    }
}

impl EndpointService {
    /// Creates an idle service; nothing touches COM until [`start`](Self::start).
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared::new()),
            control: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts the service thread and waits until the first state is published. Calling it
    /// while running returns [`Status::Running`].
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
            .name("plith-endpoints".into())
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
        self.shared.publish(&EndpointState::default());
    }

    /// Latest state; empty before `start`, after `stop` and without an audio stack.
    #[must_use]
    pub fn snapshot(&self) -> EndpointState {
        self.shared.current()
    }

    /// A channel of full states, one per change. Drain it to the latest value.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<EndpointState> {
        let (tx, rx) = channel();
        locked(&self.shared.subscribers).push(tx);
        rx
    }

    /// Pins the service to one endpoint id, or `None` to follow the Windows default again.
    /// A pin that is not active falls back to the default until it returns.
    pub fn set_pin(&self, endpoint_id: Option<String>) {
        *locked(&self.shared.pin) = endpoint_id.filter(|id| !id.is_empty());
        if let Some(tx) = locked(&self.control).as_ref() {
            let _ = tx.send(Msg::Refresh);
        }
    }
}

impl Drop for EndpointService {
    fn drop(&mut self) {
        self.stop();
    }
}
