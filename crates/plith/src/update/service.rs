//! The service thread: sleeps until the next check is due or a command arrives.

use super::{Backend, Command, Hooks, LaunchError, UpdateEvent, UpdateSettings};
use plith_core::setup::Version;
use plith_core::update::{Release, UpdateRow, next_check_in, parse_release, should_notice};
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

/// What the thread owns besides its mutable state.
pub(super) struct Job<'a> {
    pub(super) own: Version,
    pub(super) backend: &'a dyn Backend,
    pub(super) hooks: &'a Hooks,
    pub(super) events: Sender<UpdateEvent>,
}

/// The thread's mutable state.
struct State {
    enabled: bool,
    started: u64,
    last_ok: u64,
    last_failed: Option<u64>,
    seen: String,
    /// The newest release found, for `InstallReady`.
    ready: Option<Release>,
}

impl Job<'_> {
    fn emit(&self, event: UpdateEvent) {
        if self.events.send(event).is_ok() {
            (self.hooks.waker)();
        }
    }

    fn fail(&self, what: &str, reason: &str) {
        (self.hooks.on_failure)(&format!("update: {what} failed ({reason})"));
    }
}

/// Runs until `Stop` or until the service is dropped.
pub(super) fn run(job: &Job, settings: UpdateSettings, inbox: &Receiver<Command>) {
    let mut state = State {
        enabled: settings.enabled,
        started: job.backend.now(),
        last_ok: settings.checked,
        last_failed: None,
        seen: settings.seen,
        ready: None,
    };
    loop {
        let command = if state.enabled {
            let wait = next_check_in(
                job.backend.now(),
                state.started,
                state.last_ok,
                state.last_failed,
            );
            match inbox.recv_timeout(Duration::from_secs(wait)) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match inbox.recv() {
                Ok(command) => Some(command),
                Err(_) => return,
            }
        };
        match command {
            None | Some(Command::CheckNow) => check(job, &mut state),
            Some(Command::Install(release)) => install(job, &release),
            Some(Command::InstallReady) => {
                if let Some(release) = state.ready.clone() {
                    install(job, &release);
                }
            }
            Some(Command::Enable(on)) => state.enabled = on,
            Some(Command::Stop) => return,
        }
    }
}

/// One check: the row, the outcome, and a notice when it is due.
fn check(job: &Job, state: &mut State) {
    job.emit(UpdateEvent::Row(UpdateRow::Checking));
    let now = job.backend.now();
    let text = match job.backend.latest() {
        Ok(text) => text,
        Err(reason) => {
            state.last_failed = Some(now);
            job.fail("check", &reason);
            job.emit(UpdateEvent::Row(UpdateRow::Failed));
            return;
        }
    };
    state.last_ok = now;
    state.last_failed = None;
    job.emit(UpdateEvent::Checked(now));
    match text.as_deref().and_then(parse_release) {
        Some(release) if release.version > job.own => announce(job, state, release),
        _ => job.emit(UpdateEvent::Row(UpdateRow::Current)),
    }
}

/// A newer release: the row always, the notice once per version and never over a game.
fn announce(job: &Job, state: &mut State, release: Release) {
    let version = release.version;
    state.ready = Some(release.clone());
    job.emit(UpdateEvent::Row(UpdateRow::Ready(version)));
    if should_notice(&version, job.own, &state.seen) && !job.backend.busy() {
        state.seen = version.to_string();
        job.emit(UpdateEvent::Notice(release));
        job.emit(UpdateEvent::Seen(version));
    }
}

/// Download, signature check, launch. Any failure ends in a row, never in a half-run installer.
fn install(job: &Job, release: &Release) {
    let version = release.version;
    job.emit(UpdateEvent::Row(UpdateRow::Downloading(0)));
    let target = match prepare(job, version) {
        Ok(target) => target,
        Err(reason) => {
            job.fail("download", &reason);
            job.emit(UpdateEvent::Row(UpdateRow::Failed));
            return;
        }
    };
    let mut last = 0u8;
    let mut progress = |percent: u8| {
        if percent >= last.saturating_add(2) || (percent == 100 && last != 100) {
            last = percent;
            job.emit(UpdateEvent::Row(UpdateRow::Downloading(percent)));
        }
    };
    if let Err(reason) = job
        .backend
        .download(&release.setup_url, &target, &mut progress)
    {
        job.fail("download", &reason);
        job.emit(UpdateEvent::Row(UpdateRow::Failed));
        return;
    }
    match job.backend.launch(&target, job.own) {
        Ok(()) => {}
        Err(LaunchError::BadSignature) => {
            let _ = std::fs::remove_file(&target);
            job.fail("signature check", "the file is not signed by Plith");
            job.emit(UpdateEvent::Row(UpdateRow::BadSignature));
        }
        Err(LaunchError::Failed(reason)) => {
            job.fail("installer start", &reason);
            job.emit(UpdateEvent::Row(UpdateRow::Ready(version)));
        }
    }
}

/// The updates folder, cleared of older setups, and the path the new one goes to.
fn prepare(job: &Job, version: Version) -> Result<std::path::PathBuf, String> {
    let dir = job.backend.updates_dir()?;
    let parent = dir.parent().map(Path::to_path_buf);
    let check = || {
        parent
            .iter()
            .chain(std::iter::once(&dir))
            .try_for_each(|path| refuse_reparse(path))
    };
    check()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("folder: {e}"))?;
    check()?;
    remove_older(&dir, version);
    Ok(dir.join(setup_name(version)))
}

/// Refuses a folder that is a reparse point (a junction or symlink could send the download and
/// the launch somewhere else). A missing folder is fine: it is created next.
fn refuse_reparse(path: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    const REPARSE_POINT: u32 = 0x400;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_attributes() & REPARSE_POINT != 0 => {
            Err("the updates folder is a link, refused".to_string())
        }
        _ => Ok(()),
    }
}

fn setup_name(version: Version) -> String {
    format!("Plith-Setup-{version}.exe")
}

/// Deletes the setups of other versions (and their `.part` files) from `dir`. Each deleted path
/// is rebuilt from a parsed version, so only a name this code would have written is touched.
fn remove_older(dir: &Path, keep: Version) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix("Plith-Setup-") else {
            continue;
        };
        let (version, partial) = match rest.strip_suffix(".exe.part") {
            Some(v) => (v, true),
            None => match rest.strip_suffix(".exe") {
                Some(v) => (v, false),
                None => continue,
            },
        };
        let Some(old) = Version::parse(version) else {
            continue;
        };
        if old == keep && !partial {
            continue;
        }
        let mut literal = setup_name(old);
        if partial {
            literal.push_str(".part");
        }
        if literal == name {
            let _ = std::fs::remove_file(dir.join(literal));
        }
    }
}
