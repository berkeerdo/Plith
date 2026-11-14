//! `VoicemeeterService`: owns the DLL, the poll thread and the shared state.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use super::api::Api;
use super::poll;
use super::{LoginSession, Rail, VmLevels, VmSnapshot, clamp_gain, param_name};

/// Whether the Voicemeeter engine can be used right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Logged in to the engine.
    Running,
    /// Not usable; the text says why (not installed, login refused, engine stopped).
    Unavailable(String),
}

/// State shared between the service handle and its poll thread.
pub(super) struct Shared {
    pub(super) stop: AtomicBool,
    snapshot: Mutex<Option<VmSnapshot>>,
    levels: Mutex<VmLevels>,
    status: Mutex<Status>,
    subscribers: Mutex<Vec<Sender<Option<VmSnapshot>>>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            snapshot: Mutex::new(None),
            levels: Mutex::new(VmLevels::default()),
            status: Mutex::new(Status::Unavailable("not started".into())),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn set_status(&self, status: Status) {
        *locked(&self.status) = status;
    }

    pub(super) fn set_levels(&self, levels: VmLevels) {
        *locked(&self.levels) = levels;
    }

    /// Stores a new snapshot and, when it differs, sends it to every subscriber.
    pub(super) fn publish(&self, snapshot: Option<&VmSnapshot>) {
        {
            let mut current = locked(&self.snapshot);
            if current.as_ref() == snapshot {
                return;
            }
            *current = snapshot.cloned();
        }
        locked(&self.subscribers).retain(|tx| tx.send(snapshot.cloned()).is_ok());
    }
}

/// Headless Voicemeeter Remote API client with its own poll thread.
///
/// Create with [`VoicemeeterService::new`], then [`start`](Self::start). Without Voicemeeter
/// installed everything stays inert and `start` reports [`Status::Unavailable`].
pub struct VoicemeeterService {
    shared: Arc<Shared>,
    api: Mutex<Option<Arc<Api>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Default for VoicemeeterService {
    fn default() -> Self {
        Self::new()
    }
}

impl VoicemeeterService {
    /// Creates an idle service; nothing is loaded until [`start`](Self::start).
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared::new()),
            api: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Loads the DLL, logs in and starts the poll thread. Never panics: any problem comes back
    /// as [`Status::Unavailable`]. Calling it while running returns the current status.
    pub fn start(&self) -> Status {
        // Held for the whole start so two concurrent starts cannot both load and log in.
        let mut slot = locked(&self.thread);
        if slot.is_some() {
            return self.status();
        }
        let api = match Api::load() {
            Ok(api) => Arc::new(api),
            Err(reason) => return self.fail(reason),
        };
        // Defensive Logout, only ever right before Login: clears a stale login (rc -2).
        api.logout();
        let rc = api.login();
        if rc < 0 {
            return self.fail(format!("VBVMR_Login failed with code {rc}"));
        }
        let session = LoginSession::on_login(rc, Instant::now());
        self.shared.stop.store(false, Ordering::Release);
        self.shared.set_status(Status::Running);
        *locked(&self.api) = Some(Arc::clone(&api));
        let (shared, worker_api) = (Arc::clone(&self.shared), api);
        let spawned = thread::Builder::new()
            .name("plith-voicemeeter".into())
            .spawn(move || poll::run(worker_api, shared, session));
        match spawned {
            Ok(handle) => {
                *slot = Some(handle);
                Status::Running
            }
            Err(err) => {
                if let Some(api) = locked(&self.api).take() {
                    api.logout();
                }
                self.fail(format!("could not start the poll thread: {err}"))
            }
        }
    }

    fn fail(&self, reason: String) -> Status {
        let status = Status::Unavailable(reason);
        self.shared.set_status(status.clone());
        status
    }

    /// Stops the poll thread, waits for it, then logs out. Safe to call twice.
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(handle) = locked(&self.thread).take() {
            let _ = handle.join();
        }
        if let Some(api) = locked(&self.api).take() {
            api.logout();
        }
        self.shared.publish(None);
        self.shared.set_levels(VmLevels::default());
        self.shared
            .set_status(Status::Unavailable("stopped".into()));
    }

    /// Current status.
    #[must_use]
    pub fn status(&self) -> Status {
        locked(&self.shared.status).clone()
    }

    /// Latest snapshot, `None` before the first read or while the engine is unavailable.
    #[must_use]
    pub fn snapshot(&self) -> Option<VmSnapshot> {
        locked(&self.shared.snapshot).clone()
    }

    /// A channel of full snapshots, one per change; `None` means the engine went away.
    /// Drain it to the latest value.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Option<VmSnapshot>> {
        let (tx, rx) = channel();
        locked(&self.shared.subscribers).push(tx);
        rx
    }

    /// Latest peak levels (read only); all zero while unavailable.
    #[must_use]
    pub fn levels(&self) -> VmLevels {
        locked(&self.shared.levels).clone()
    }

    fn api(&self) -> Option<Arc<Api>> {
        locked(&self.api).clone()
    }

    /// True when `index` exists on `rail` for the running edition.
    fn in_range(&self, rail: Rail, index: usize) -> bool {
        self.snapshot().is_some_and(|s| match rail {
            Rail::Strip => index < s.kind.strip_count(),
            Rail::Bus => index < s.kind.bus_count(),
        })
    }

    /// Writes a gain in dB, clamped to the accepted range. False when unavailable, out of range
    /// or refused. The poll notices the change; nothing is echoed back here.
    pub fn set_gain(&self, rail: Rail, index: usize, gain_db: f32) -> bool {
        let Some(api) = self.api() else { return false };
        self.in_range(rail, index)
            && api.set_float(&param_name(rail, index, "Gain"), clamp_gain(gain_db))
    }

    /// Flips the mute: reads the real state, writes the opposite and returns it. `None` when
    /// unavailable, out of range or the read or write failed.
    pub fn toggle_mute(&self, rail: Rail, index: usize) -> Option<bool> {
        let api = self.api()?;
        if !self.in_range(rail, index) {
            return None;
        }
        api.toggle_float(&param_name(rail, index, "Mute"))
    }
}

impl Drop for VoicemeeterService {
    fn drop(&mut self) {
        self.stop();
    }
}
