//! `EndpointService`: one thread owns every COM attach and detach; the handle only reads
//! shared state and posts messages.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use windows::Win32::Media::Audio::{
    IMMDeviceEnumerator, IMMNotificationClient, eMultimedia, eRender,
};

use super::com::{Attachment, ComGuard, create_enumerator, default_id, list_active};
use super::notify::{DeviceEvents, Msg, ask};
use super::policy::set_default_endpoint;
use super::{AttachedState, EndpointState, resolve_target};
use crate::waker::{Waker, publish_with};

/// Whether the Core Audio stack could be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The service thread is running and registered for notifications.
    Running,
    /// Not usable; the text says why. The snapshot stays empty.
    Unavailable(String),
}

/// Status of a service from its worker thread: `None` when there is no thread, otherwise
/// whether it has ended. A worker that ended without `stop` must not look like `Running`.
pub(crate) fn worker_status(worker: Option<bool>) -> Status {
    match worker {
        Some(false) => Status::Running,
        Some(true) => Status::Unavailable("the service thread stopped unexpectedly".into()),
        None => Status::Unavailable("not running".into()),
    }
}

/// Reaps a worker that ended without `stop`: joins it and empties the slot. Returns true only
/// while a live worker occupies the slot, so a dead one never looks `Running` and `start` can
/// begin a fresh worker.
pub(crate) fn slot_alive(slot: &mut Option<JoinHandle<()>>) -> bool {
    if slot.as_ref().is_some_and(|handle| !handle.is_finished()) {
        return true;
    }
    if let Some(handle) = slot.take() {
        let _ = handle.join();
    }
    false
}

/// State shared between the handle and the service thread. Every lock is held only for a
/// plain read or write, never across a COM call.
struct Shared {
    stop: AtomicBool,
    state: Mutex<EndpointState>,
    pin: Mutex<Option<String>>,
    subscribers: Mutex<Vec<Sender<EndpointState>>>,
    waker: Waker,
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
            waker: Waker::default(),
        }
    }

    /// Stores `next` and, when it differs from the current state, sends it to subscribers and
    /// wakes the consumer.
    fn publish(&self, next: &EndpointState) {
        publish_with(&self.state, &self.subscribers, &self.waker, |current| {
            if *current == *next {
                return false;
            }
            current.clone_from(next);
            true
        });
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
    let client: IMMNotificationClient = DeviceEvents {
        tx: tx.clone(),
        flow: eRender,
        role: eMultimedia,
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
            Msg::SetVolume { value, reply } => {
                let _ = reply.send(
                    worker
                        .attachment
                        .as_ref()
                        .is_some_and(|a| a.set_volume(value)),
                );
            }
            Msg::ToggleMute { reply } => {
                let _ = reply.send(worker.attachment.as_ref().and_then(Attachment::toggle_mute));
            }
            Msg::SetDefault { id, reply } => {
                let done = set_default_endpoint(&id);
                // Also after a failure: one role may have moved, so the state must be re-read.
                worker.refresh();
                let _ = reply.send(done);
            }
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
    ///
    /// Blocks until the worker has initialized COM and read the first state (short, normally
    /// well under a second), so call it off the UI thread.
    pub fn start(&self) -> Status {
        let mut slot = locked(&self.thread);
        if slot_alive(&mut slot) {
            return Status::Running;
        }
        *locked(&self.control) = None;
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
                *locked(&self.control) = Some(tx.clone());
                *slot = Some(handle);
                // A `set_pin` that landed before the control channel existed only stored the pin;
                // the worker may already have read the old one, so ask for one more refresh.
                let _ = tx.send(Msg::Refresh);
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
        // The thread lock comes first: a concurrent `start` holds it until the worker is ready,
        // so the `Stop` below always reaches a worker that exists (no join that waits forever).
        let mut slot = locked(&self.thread);
        self.shared.stop.store(true, Ordering::Release);
        if let Some(tx) = locked(&self.control).take() {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(handle) = slot.take() {
            let _ = handle.join();
        }
        drop(slot);
        self.shared.publish(&EndpointState::default());
    }

    /// Whether the worker thread is up. A thread that ended without `stop` reports
    /// unavailable, never running.
    #[must_use]
    pub fn status(&self) -> Status {
        match self.thread.try_lock() {
            Ok(slot) => worker_status(slot.as_ref().map(JoinHandle::is_finished)),
            Err(_) => Status::Unavailable("starting or stopping".into()),
        }
    }

    /// Registers a callback run on the publishing thread after every published change, outside
    /// every lock. It must not block; use it to post a message to a UI thread. Replaces any
    /// earlier callback.
    pub fn set_waker(&self, waker: Box<dyn Fn() + Send + Sync>) {
        self.shared.waker.set(waker);
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

    /// Sets the attached endpoint's master volume. The value is clamped to 0.0..=1.0; NaN is
    /// refused. Returns whether a value was written (`false` with nothing attached).
    pub fn set_volume(&self, value: f32) -> bool {
        self.request(|reply| Msg::SetVolume { value, reply })
            .unwrap_or(false)
    }

    /// Reads the attached endpoint's mute and writes the opposite. Returns the new state, or
    /// `None` when nothing is attached or the write failed.
    pub fn toggle_mute(&self) -> Option<bool> {
        self.request(|reply| Msg::ToggleMute { reply }).flatten()
    }

    /// Makes `endpoint_id` the Windows default render device for the Console and Multimedia
    /// roles (never Communications). Returns whether both writes succeeded; on `false` one role
    /// may still have moved, and the snapshot shows what Windows now reports.
    pub fn set_default_endpoint(&self, endpoint_id: &str) -> bool {
        if endpoint_id.trim().is_empty() {
            return false;
        }
        let id = endpoint_id.to_owned();
        self.request(|reply| Msg::SetDefault { id, reply })
            .unwrap_or(false)
    }

    fn request<T>(&self, make: impl FnOnce(SyncSender<T>) -> Msg) -> Option<T> {
        let tx = locked(&self.control).clone()?;
        ask(&tx, make)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn waker_fires_per_distinct_publish_only() {
        let shared = Shared::new();
        let count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&count);
        shared.waker.set(Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        let state = EndpointState {
            default_id: Some("a".into()),
            ..EndpointState::default()
        };
        shared.publish(&state);
        shared.publish(&state);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        shared.publish(&EndpointState::default());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn dead_worker_is_not_running() {
        assert_eq!(worker_status(Some(false)), Status::Running);
        assert!(matches!(worker_status(Some(true)), Status::Unavailable(_)));
        assert!(matches!(worker_status(None), Status::Unavailable(_)));
    }

    #[test]
    fn slot_alive_reaps_a_finished_worker() {
        let mut empty: Option<JoinHandle<()>> = None;
        assert!(!slot_alive(&mut empty));
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let mut live = Some(thread::spawn(move || {
            let _ = release_rx.recv();
        }));
        assert!(slot_alive(&mut live));
        assert!(live.is_some());
        let _ = release_tx.send(());
        let mut dead = live.take();
        while dead.as_ref().is_some_and(|h| !h.is_finished()) {
            thread::yield_now();
        }
        assert!(!slot_alive(&mut dead));
        assert!(dead.is_none());
    }
}
