//! `BrightnessService`: one worker thread owns discovery, every device and every write.
//!
//! The worker initializes COM (WMI needs it), finds the displays that answer a read (built-in
//! panel first), and runs the [`CoalescingWriter`] pump as a job on its own queue, so a
//! 56 ms DDC/CI write never blocks a caller. A second thread listens for
//! `WmiMonitorBrightnessEvent` when a built-in panel exists and forwards the new level.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use super::discovery::{apply_event, apply_write, keep_answering};
use super::run_token::RunTokens;
use super::wql::panel_id;
use super::writer::{CoalescingWriter, Job};
use super::{BrightnessDevice, DeviceLevel, Status, ddc, wmi};
use crate::waker::{Waker, publish_with};

/// How long `start` waits for discovery (each DDC read is tens of milliseconds).
const READY_CAP: Duration = Duration::from_secs(15);

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// State shared between the handle and the worker.
struct Shared {
    stop: AtomicBool,
    snapshot: Mutex<Vec<DeviceLevel>>,
    subscribers: Mutex<Vec<Sender<Vec<DeviceLevel>>>>,
    waker: Waker,
    runs: RunTokens,
}

impl Shared {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            snapshot: Mutex::new(Vec::new()),
            subscribers: Mutex::new(Vec::new()),
            waker: Waker::default(),
            runs: RunTokens::default(),
        }
    }

    /// Applies `change` to the snapshot on behalf of run `token` and, when it reports a change,
    /// sends the new snapshot to every subscriber and wakes the consumer. A run that is no
    /// longer the newest (a discovery that outlived a `start` timeout) changes nothing.
    fn update(&self, token: u64, change: impl FnOnce(&mut Vec<DeviceLevel>) -> bool) {
        publish_with(&self.snapshot, &self.subscribers, &self.waker, |current| {
            self.runs.is_current(token) && change(current)
        });
    }

    fn replace(&self, token: u64, levels: Vec<DeviceLevel>) {
        self.update(token, |current| {
            let changed = *current != levels;
            *current = levels;
            changed
        });
    }

    /// Whether the worker of run `token` should end: stopped, or replaced by a newer run.
    fn should_end(&self, token: u64) -> bool {
        self.stop.load(Ordering::Acquire) || !self.runs.is_current(token)
    }
}

enum Msg {
    /// A caller asked for a level.
    Set(u8),
    /// The writer's pump, queued by its runner.
    Job(Job),
    /// A built-in panel changed level (the Settings slider, a key, another app).
    Event {
        id: String,
        percent: u8,
    },
    Stop,
}

/// Owns COM for one thread; dropped last.
struct ComGuard;

impl ComGuard {
    fn init() -> windows::core::Result<Self> {
        // SAFETY: plain apartment initialization on the calling thread, balanced by `Drop`.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: balances the successful `CoInitializeEx` in `init`.
        unsafe { CoUninitialize() };
    }
}

/// The event listener thread, stopped and joined on drop.
struct Listener {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    fn spawn(tx: Sender<Msg>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("plith-brightness-events".into())
            .spawn(move || {
                let Ok(_com) = ComGuard::init() else {
                    return;
                };
                wmi::listen(&flag, |instance, percent| {
                    let _ = tx.send(Msg::Event {
                        id: panel_id(&instance),
                        percent,
                    });
                });
            })
            .ok();
        Self { stop, thread }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Built-in panels first (the first device is "the screen" on a laptop), then external ones.
/// Returns the candidates and how many are built-in panels.
fn candidates() -> (Vec<Box<dyn BrightnessDevice>>, usize) {
    let mut all = wmi::enumerate();
    let panels = all.len();
    all.extend(ddc::discover());
    (all, panels)
}

fn run(
    shared: &Arc<Shared>,
    token: u64,
    tx: Sender<Msg>,
    rx: &Receiver<Msg>,
    ready: &SyncSender<Status>,
) {
    let _com = match ComGuard::init() {
        Ok(guard) => guard,
        Err(err) => {
            let _ = ready.send(Status::Unavailable(format!(
                "COM initialization failed: {err}"
            )));
            return;
        }
    };
    let (all, panels) = candidates();
    let found = keep_answering(all);
    shared.replace(token, found.levels);
    let queue = tx.clone();
    let observed = Arc::clone(shared);
    let writer = CoalescingWriter::new(
        found.devices,
        Box::new(move |job| {
            let _ = queue.send(Msg::Job(job));
        }),
        Box::new(move |outcome| {
            observed.update(token, |levels| {
                apply_write(levels, outcome.value, &outcome.failed)
            });
        }),
    );
    let listener = (panels > 0).then(|| Listener::spawn(tx));
    let _ = ready.send(Status::Running);
    serve(shared, token, &writer, rx);
    // Stop the listener before the devices (inside `writer`) are dropped.
    drop(listener);
}

fn serve(shared: &Shared, token: u64, writer: &CoalescingWriter, rx: &Receiver<Msg>) {
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Set(percent) => writer.request(percent),
            Msg::Job(job) => job(),
            Msg::Event { id, percent } => {
                shared.update(token, |levels| apply_event(levels, &id, percent));
            }
            Msg::Stop => break,
        }
        if shared.should_end(token) {
            break;
        }
    }
}

/// Headless brightness client with its own worker thread.
///
/// Create with [`BrightnessService::new`], then [`start`](Self::start). A desktop with no
/// built-in panel and no DDC/CI monitor is a normal empty state: `Running` with an empty
/// snapshot. [`set_percent`](Self::set_percent) is the only call that writes to hardware.
pub struct BrightnessService {
    shared: Arc<Shared>,
    control: Mutex<Option<Sender<Msg>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Default for BrightnessService {
    fn default() -> Self {
        Self::new()
    }
}

impl BrightnessService {
    /// Creates an idle service; nothing touches a display until [`start`](Self::start).
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared::new()),
            control: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts the worker and waits for discovery. Calling it while running returns
    /// [`Status::Running`].
    ///
    /// Blocks until discovery is done: up to 15 s (the cap; a typical machine needs well under
    /// a second, each DDC/CI read takes tens of milliseconds). Call it off the UI thread.
    pub fn start(&self) -> Status {
        let mut slot = locked(&self.thread);
        if slot.is_some() {
            return Status::Running;
        }
        self.shared.stop.store(false, Ordering::Release);
        // A new run makes any zombie from an earlier timed-out start stale: it can no longer
        // publish and ends on its next message.
        let token = self.shared.runs.begin();
        let (tx, rx) = channel();
        let (ready_tx, ready_rx) = sync_channel(1);
        let (shared, worker_tx) = (Arc::clone(&self.shared), tx.clone());
        let spawned = thread::Builder::new()
            .name("plith-brightness".into())
            .spawn(move || run(&shared, token, worker_tx, &rx, &ready_tx));
        let handle = match spawned {
            Ok(handle) => handle,
            Err(err) => return Status::Unavailable(format!("could not start the thread: {err}")),
        };
        match ready_rx.recv_timeout(READY_CAP) {
            Ok(Status::Running) => {
                *locked(&self.control) = Some(tx);
                *slot = Some(handle);
                Status::Running
            }
            Ok(unavailable) => {
                let _ = handle.join();
                unavailable
            }
            Err(RecvTimeoutError::Timeout) => {
                // The worker is stuck in a driver call; let it end on its own. Its run is
                // retired now, so a late discovery cannot publish a stale snapshot.
                self.shared.runs.begin();
                let _ = tx.send(Msg::Stop);
                Status::Unavailable("display discovery did not finish in time".into())
            }
            Err(RecvTimeoutError::Disconnected) => {
                // The worker ended (a panic) before it reported; it is already gone.
                let _ = handle.join();
                Status::Unavailable("the worker thread ended before discovery finished".into())
            }
        }
    }

    /// Stops the worker (after any write in flight) and closes every display handle. Safe to
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
        self.shared.replace(self.shared.runs.latest(), Vec::new());
    }

    /// Whether the worker thread is up. A worker that ended without `stop` reports unavailable,
    /// never running.
    #[must_use]
    pub fn status(&self) -> Status {
        match self.thread.try_lock() {
            Ok(slot) => match slot.as_ref().map(JoinHandle::is_finished) {
                Some(false) => Status::Running,
                Some(true) => Status::Unavailable("the worker thread stopped unexpectedly".into()),
                None => Status::Unavailable("not running".into()),
            },
            Err(_) => Status::Unavailable("starting or stopping".into()),
        }
    }

    /// Registers a callback run on the publishing thread after every published change, outside
    /// every lock. It must not block; use it to post a message to a UI thread. Replaces any
    /// earlier callback.
    pub fn set_waker(&self, waker: Box<dyn Fn() + Send + Sync>) {
        self.shared.waker.set(waker);
    }

    /// The latest levels, built-in panel first; empty when there is no controllable display or
    /// the service is not started.
    #[must_use]
    pub fn snapshot(&self) -> Vec<DeviceLevel> {
        locked(&self.shared.snapshot).clone()
    }

    /// A channel of snapshots, one per change. Drain it to the latest value.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Vec<DeviceLevel>> {
        let (tx, rx) = channel();
        locked(&self.shared.subscribers).push(tx);
        rx
    }

    /// Asks every display to go to `percent` (clamped to 100). Returns at once: the newest value
    /// wins and the write happens on the worker. `false` when the service is not running.
    pub fn set_percent(&self, percent: u8) -> bool {
        locked(&self.control)
            .as_ref()
            .is_some_and(|tx| tx.send(Msg::Set(percent)).is_ok())
    }
}

impl Drop for BrightnessService {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn level(percent: u8) -> DeviceLevel {
        DeviceLevel {
            id: "d".into(),
            name: "D".into(),
            percent,
        }
    }

    #[test]
    fn waker_fires_per_distinct_change_only() {
        let shared = Shared::new();
        let token = shared.runs.begin();
        let count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&count);
        shared.waker.set(Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        shared.replace(token, vec![level(40)]);
        shared.replace(token, vec![level(40)]);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        shared.replace(token, vec![level(50)]);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_stale_run_cannot_overwrite_a_newer_snapshot() {
        let shared = Shared::new();
        let zombie = shared.runs.begin();
        let fresh = shared.runs.begin();
        shared.replace(fresh, vec![level(70)]);
        shared.replace(zombie, vec![level(10)]);
        assert_eq!(locked(&shared.snapshot)[0].percent, 70);
        assert!(shared.should_end(zombie));
        assert!(!shared.should_end(fresh));
    }
}
