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
use super::wql::panel_id;
use super::writer::{CoalescingWriter, Job};
use super::{BrightnessDevice, DeviceLevel, Status, ddc, wmi};

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
}

impl Shared {
    /// Applies `change` to the snapshot and, when it reports a change, sends the new snapshot to
    /// every subscriber.
    fn update(&self, change: impl FnOnce(&mut Vec<DeviceLevel>) -> bool) {
        let next = {
            let mut current = locked(&self.snapshot);
            if !change(&mut current) {
                return;
            }
            current.clone()
        };
        locked(&self.subscribers).retain(|tx| tx.send(next.clone()).is_ok());
    }

    fn replace(&self, levels: Vec<DeviceLevel>) {
        self.update(|current| {
            let changed = *current != levels;
            *current = levels;
            changed
        });
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

fn run(shared: &Arc<Shared>, tx: Sender<Msg>, rx: &Receiver<Msg>, ready: &SyncSender<Status>) {
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
    shared.replace(found.levels);
    let queue = tx.clone();
    let observed = Arc::clone(shared);
    let writer = CoalescingWriter::new(
        found.devices,
        Box::new(move |job| {
            let _ = queue.send(Msg::Job(job));
        }),
        Box::new(move |outcome| {
            observed.update(|levels| apply_write(levels, outcome.value, &outcome.failed));
        }),
    );
    let listener = (panels > 0).then(|| Listener::spawn(tx));
    let _ = ready.send(Status::Running);
    serve(shared, &writer, rx);
    // Stop the listener before the devices (inside `writer`) are dropped.
    drop(listener);
}

fn serve(shared: &Shared, writer: &CoalescingWriter, rx: &Receiver<Msg>) {
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Set(percent) => writer.request(percent),
            Msg::Job(job) => job(),
            Msg::Event { id, percent } => shared.update(|levels| apply_event(levels, &id, percent)),
            Msg::Stop => break,
        }
        if shared.stop.load(Ordering::Acquire) {
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
            shared: Arc::new(Shared {
                stop: AtomicBool::new(false),
                snapshot: Mutex::new(Vec::new()),
                subscribers: Mutex::new(Vec::new()),
            }),
            control: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts the worker and waits for discovery. Calling it while running returns
    /// [`Status::Running`].
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
            .name("plith-brightness".into())
            .spawn(move || run(&shared, worker_tx, &rx, &ready_tx));
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
                // The worker is stuck in a driver call; let it end on its own.
                self.shared.stop.store(true, Ordering::Release);
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
        self.shared.stop.store(true, Ordering::Release);
        if let Some(tx) = locked(&self.control).take() {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(handle) = locked(&self.thread).take() {
            let _ = handle.join();
        }
        self.shared.replace(Vec::new());
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
