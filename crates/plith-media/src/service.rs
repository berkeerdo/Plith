//! `MediaService`: one worker thread owns the SMTC manager, the current session's event
//! registrations and every blocking `WinRT` call.
//!
//! `WinRT` events arrive on threadpool threads. Their handlers only bump the [`Generation`] and
//! post a message to the worker; all reading happens on the worker, which may block on an
//! operation (capped at 2 s) without ever stalling a callback thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use crate::coalesce::Generation;
use crate::session::{self, Command};
use crate::snapshot::{MediaSnapshot, PlaybackFacts, SessionInfo, Status, TrackFacts};
use crate::timeline::Timeline;

/// How long a caller waits for the worker to answer a command or a session list.
const ASK_CAP: Duration = Duration::from_secs(6);

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// State shared between the handle and the worker.
struct Shared {
    stop: AtomicBool,
    generation: Generation,
    snapshot: Mutex<MediaSnapshot>,
    subscribers: Mutex<Vec<Sender<MediaSnapshot>>>,
}

impl Shared {
    /// Stores a new snapshot and, when it differs, sends it to every subscriber.
    fn publish(&self, next: &MediaSnapshot) {
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

/// What the worker is asked to do. Events carry no data: the worker re-reads.
enum Msg {
    /// The current session was replaced or went away.
    SessionChanged,
    /// Title, artist or artwork changed (the only reason to re-read the thumbnail).
    Properties,
    /// Playing, paused or capabilities changed.
    Playback,
    /// The position moved (about once a second on some sources).
    Timeline,
    Sessions(Sender<Vec<SessionInfo>>),
    Command(Command, Sender<bool>),
    Stop,
}

/// Which reads a batch of messages calls for.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)] // independent flags that mirror SMTC
struct Pending {
    session: bool,
    properties: bool,
    playback: bool,
    timeline: bool,
    stop: bool,
}

/// Event registrations on one session, removed when the session is replaced.
struct Attached {
    session: Session,
    tokens: [i64; 3],
}

impl Attached {
    fn detach(self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.tokens[0]);
        let _ = self.session.RemovePlaybackInfoChanged(self.tokens[1]);
        let _ = self.session.RemoveTimelinePropertiesChanged(self.tokens[2]);
    }
}

/// Builds a handler that bumps the generation and posts `msg()`.
fn poster<S, A>(shared: &Arc<Shared>, tx: &Sender<Msg>, msg: fn() -> Msg) -> TypedEventHandler<S, A>
where
    S: windows::core::RuntimeType + 'static,
    A: windows::core::RuntimeType + 'static,
{
    let (shared, tx) = (Arc::clone(shared), tx.clone());
    TypedEventHandler::new(move |_, _| {
        shared.generation.bump();
        let _ = tx.send(msg());
        Ok(())
    })
}

struct Worker {
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    manager: Manager,
    manager_token: i64,
    attached: Option<Attached>,
    track: Option<TrackFacts>,
    playback: Option<PlaybackFacts>,
    timeline: Option<Timeline>,
}

impl Worker {
    fn attach_current(&mut self) {
        self.track = None;
        self.playback = None;
        self.timeline = None;
        let Ok(session) = self.manager.GetCurrentSession() else {
            return;
        };
        let (shared, tx) = (&self.shared, &self.tx);
        let tokens = (|| {
            Ok::<_, windows::core::Error>([
                session.MediaPropertiesChanged(&poster(shared, tx, || Msg::Properties))?,
                session.PlaybackInfoChanged(&poster(shared, tx, || Msg::Playback))?,
                session.TimelinePropertiesChanged(&poster(shared, tx, || Msg::Timeline))?,
            ])
        })();
        if let Ok(tokens) = tokens {
            self.attached = Some(Attached { session, tokens });
        }
    }

    fn detach_current(&mut self) {
        if let Some(attached) = self.attached.take() {
            attached.detach();
        }
    }

    fn handle(&self, msg: Msg, pending: &mut Pending) {
        match msg {
            Msg::SessionChanged => pending.session = true,
            Msg::Properties => pending.properties = true,
            Msg::Playback => pending.playback = true,
            Msg::Timeline => pending.timeline = true,
            Msg::Stop => pending.stop = true,
            Msg::Sessions(reply) => {
                let _ = reply.send(session::list_sessions(&self.manager));
            }
            Msg::Command(command, reply) => {
                let done = self
                    .attached
                    .as_ref()
                    .is_some_and(|a| session::run_command(&a.session, command));
                let _ = reply.send(done);
            }
        }
    }

    /// Applies one coalesced batch: re-attach if the session changed, then re-read only what
    /// the batch calls for. A read that a newer event overtook is dropped; that event's own
    /// message is already queued and triggers the next read.
    fn apply(&mut self, pending: &Pending) {
        let seen = self.shared.generation.current();
        if pending.session {
            self.detach_current();
            self.attach_current();
        }
        let fresh = pending.session;
        if let Some(attached) = &self.attached {
            let session = &attached.session;
            if fresh || pending.properties {
                self.track = session::read_track(session);
            }
            if fresh || pending.properties || pending.playback {
                self.playback = session::read_playback(session);
            }
            // Position follows any change: a pause or seek moves it without a timeline event.
            self.timeline = session::read_timeline(session);
        }
        if self.shared.generation.is_current(seen) {
            self.shared.publish(&self.current());
        }
    }

    fn current(&self) -> MediaSnapshot {
        match &self.attached {
            Some(attached) => MediaSnapshot::assemble(
                &session::source_id(&attached.session),
                self.track.clone(),
                self.playback,
                self.timeline,
            ),
            None => MediaSnapshot::none(),
        }
    }
}

/// Owns COM for the worker thread; dropped last.
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

fn run(shared: Arc<Shared>, tx: Sender<Msg>, rx: &Receiver<Msg>, ready: &SyncSender<Status>) {
    let _com = match ComGuard::init() {
        Ok(guard) => guard,
        Err(err) => {
            let _ = ready.send(Status::Unavailable(format!(
                "COM initialization failed: {err}"
            )));
            return;
        }
    };
    let manager = match session::request_manager() {
        Ok(manager) => manager,
        Err(reason) => {
            let _ = ready.send(Status::Unavailable(reason));
            return;
        }
    };
    let token = manager.CurrentSessionChanged(&poster(&shared, &tx, || Msg::SessionChanged));
    let Ok(manager_token) = token else {
        let _ = ready.send(Status::Unavailable("could not subscribe to SMTC".into()));
        return;
    };
    let mut worker = Worker {
        shared,
        tx,
        manager,
        manager_token,
        attached: None,
        track: None,
        playback: None,
        timeline: None,
    };
    worker.apply(&Pending {
        session: true,
        ..Pending::default()
    });
    let _ = ready.send(Status::Running);
    serve(&mut worker, rx);
    // Shutdown on this thread, so no callback registration outlives the manager.
    worker.detach_current();
    let _ = worker
        .manager
        .RemoveCurrentSessionChanged(worker.manager_token);
}

fn serve(worker: &mut Worker, rx: &Receiver<Msg>) {
    while let Ok(first) = rx.recv() {
        let mut pending = Pending::default();
        worker.handle(first, &mut pending);
        // A burst of events becomes one read.
        while let Ok(next) = rx.try_recv() {
            worker.handle(next, &mut pending);
        }
        if pending.stop || worker.shared.stop.load(Ordering::Acquire) {
            break;
        }
        if pending.session || pending.properties || pending.playback || pending.timeline {
            worker.apply(&pending);
        }
    }
}

/// Headless SMTC client with its own worker thread.
///
/// Create with [`MediaService::new`], then [`start`](Self::start). With no session the snapshot
/// says `has_session: false`; with SMTC unavailable `start` returns [`Status::Unavailable`].
/// Nothing panics when a session dies mid-read.
pub struct MediaService {
    shared: Arc<Shared>,
    control: Mutex<Option<Sender<Msg>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Default for MediaService {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaService {
    /// Creates an idle service; nothing touches `WinRT` until [`start`](Self::start).
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                stop: AtomicBool::new(false),
                generation: Generation::default(),
                snapshot: Mutex::new(MediaSnapshot::none()),
                subscribers: Mutex::new(Vec::new()),
            }),
            control: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts the worker and waits for the first reading. Calling it while running returns
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
            .name("plith-media".into())
            .spawn(move || run(shared, worker_tx, &rx, &ready_tx));
        let handle = match spawned {
            Ok(handle) => handle,
            Err(err) => return Status::Unavailable(format!("could not start the thread: {err}")),
        };
        match ready_rx.recv() {
            Ok(Status::Running) => {
                *locked(&self.control) = Some(tx);
                *slot = Some(handle);
                Status::Running
            }
            Ok(unavailable) => {
                let _ = handle.join();
                unavailable
            }
            Err(_) => {
                let _ = handle.join();
                Status::Unavailable("the worker thread ended before it was ready".into())
            }
        }
    }

    /// Stops the worker, which removes its event registrations itself, and waits for it. Safe
    /// to call twice.
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(tx) = locked(&self.control).take() {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(handle) = locked(&self.thread).take() {
            let _ = handle.join();
        }
        self.shared.publish(&MediaSnapshot::none());
    }

    /// The latest reading; `has_session: false` when nothing is playing or not started.
    #[must_use]
    pub fn snapshot(&self) -> MediaSnapshot {
        locked(&self.shared.snapshot).clone()
    }

    /// A channel of readings, one per change. Drain it to the latest value.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<MediaSnapshot> {
        let (tx, rx) = channel();
        locked(&self.shared.subscribers).push(tx);
        rx
    }

    /// Every SMTC session (not just the current one); empty when stopped or none exist.
    #[must_use]
    pub fn sessions(&self) -> Vec<SessionInfo> {
        self.ask(Msg::Sessions).unwrap_or_default()
    }

    /// Toggles play and pause on the current session; `false` when there is none, the source
    /// refuses or it does not answer in 2 s.
    pub fn toggle_play_pause(&self) -> bool {
        self.command(Command::TogglePlayPause)
    }

    /// Skips to the next track; `false` as for [`toggle_play_pause`](Self::toggle_play_pause).
    pub fn next(&self) -> bool {
        self.command(Command::Next)
    }

    /// Skips to the previous track; `false` as for [`toggle_play_pause`](Self::toggle_play_pause).
    pub fn previous(&self) -> bool {
        self.command(Command::Previous)
    }

    /// Seeks to `position` within the track (start already accounted for); `false` as for
    /// [`toggle_play_pause`](Self::toggle_play_pause).
    pub fn seek(&self, position: Duration) -> bool {
        self.command(Command::Seek(position))
    }

    fn command(&self, command: Command) -> bool {
        self.ask(|reply| Msg::Command(command, reply))
            .unwrap_or(false)
    }

    /// Sends a request that carries a reply channel and waits for the answer.
    fn ask<T>(&self, build: impl FnOnce(Sender<T>) -> Msg) -> Option<T> {
        let tx = locked(&self.control).clone()?;
        let (reply_tx, reply_rx) = channel();
        tx.send(build(reply_tx)).ok()?;
        reply_rx.recv_timeout(ASK_CAP).ok()
    }
}

impl Drop for MediaService {
    fn drop(&mut self) {
        self.stop();
    }
}
