//! The update service: a daily check of the latest GitHub release, the download of a newer
//! setup, the signature check and the start of the installer. It runs on its own thread, sleeps
//! between checks and wakes the UI thread only to deliver an event. Nothing installs without a
//! click: [`UpdateService::install_ready`] is the only way to download.
//!
//! The network, the clock, the busy check and the launch sit behind [`Backend`], so tests run
//! without any of them.

mod launch;
mod link;
mod real;
mod service;

#[cfg(test)]
mod tests;

pub use link::UpdateLink;
pub use real::RealBackend;

use plith_core::setup::Version;
use plith_core::update::{Release, UpdateRow};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// What the service tells the UI thread; the app persists `Checked` and `Seen`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEvent {
    /// The About row changed.
    Row(UpdateRow),
    /// A newer release should be announced to the user once.
    Notice(Release),
    /// A check succeeded at this Unix time (seconds).
    Checked(u64),
    /// The user was told about this version.
    Seen(Version),
}

/// The settings the service starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSettings {
    /// Whether the background check runs.
    pub enabled: bool,
    /// Unix time of the last successful check; 0 for never.
    pub checked: u64,
    /// The last version the user was told about; empty for none.
    pub seen: String,
}

/// The callbacks the service thread needs.
pub struct Hooks {
    /// Runs after each event is queued; it must only post a wake and touch no app state.
    pub waker: Box<dyn Fn() + Send>,
    /// Runs with a line safe to log after a failure.
    pub on_failure: Box<dyn Fn(&str) + Send>,
}

/// Why the downloaded installer was not started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The file is not signed by the Plith certificate; the caller deletes it.
    BadSignature,
    /// The installer did not start (for example the user declined the UAC prompt).
    Failed(String),
    /// The saved file is not where it was saved (a link on the way, a network path); starting
    /// it is refused, and downloading again would end the same way.
    Refused(String),
}

/// What the service takes from the outside world.
pub trait Backend: Send + Sync {
    /// The latest-release JSON; `None` when no release exists yet (a 404).
    ///
    /// # Errors
    /// A short message when the request failed.
    fn latest(&self) -> Result<Option<String>, String>;

    /// Downloads `url` to `dest`, reporting whole percents.
    ///
    /// # Errors
    /// A short message when the download failed.
    fn download(&self, url: &str, dest: &Path, progress: &mut dyn FnMut(u8)) -> Result<(), String>;

    /// Verifies `setup` is signed by Plith and, only then, starts it with `--update --from
    /// <own>`.
    ///
    /// # Errors
    /// [`LaunchError`] saying whether the signature failed or the start did.
    fn launch(&self, setup: &Path, own: Version) -> Result<(), LaunchError>;

    /// Whether the user is busy (a game, a presentation); a notice then waits.
    fn busy(&self) -> bool;

    /// Unix time in seconds.
    fn now(&self) -> u64;

    /// The folder downloads go to.
    ///
    /// # Errors
    /// A short message when the folder is unknown.
    fn updates_dir(&self) -> Result<PathBuf, String>;
}

enum Command {
    CheckNow,
    #[cfg(test)]
    Install(Release),
    InstallReady,
    Enable(bool),
    Stop,
}

/// The running service.
pub struct UpdateService {
    commands: Sender<Command>,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "only the test-only stop joins it")
    )]
    worker: Option<JoinHandle<()>>,
}

impl UpdateService {
    /// Starts the thread over the real network and system. Returns at once.
    #[must_use]
    pub fn start(
        own: Version,
        settings: UpdateSettings,
        hooks: Hooks,
    ) -> (Self, Receiver<UpdateEvent>) {
        Self::start_over(own, settings, hooks, Box::new(RealBackend))
    }

    /// Starts the thread over any [`Backend`].
    #[must_use]
    pub fn start_over(
        own: Version,
        settings: UpdateSettings,
        hooks: Hooks,
        backend: Box<dyn Backend>,
    ) -> (Self, Receiver<UpdateEvent>) {
        let (events, feed) = channel();
        let (commands, inbox) = channel();
        let worker = std::thread::Builder::new()
            .name("plith-update".to_string())
            .spawn(move || {
                service::run(
                    &service::Job {
                        own,
                        backend: backend.as_ref(),
                        hooks: &hooks,
                        events,
                    },
                    settings,
                    &inbox,
                );
            })
            .ok();
        (Self { commands, worker }, feed)
    }

    /// Checks now, whatever the setting says.
    pub fn check_now(&self) {
        let _ = self.commands.send(Command::CheckNow);
    }

    #[cfg(test)]
    /// Downloads, verifies and starts the installer of `release`.
    pub fn install(&self, release: Release) {
        let _ = self.commands.send(Command::Install(release));
    }

    /// Like [`Self::install`], for the newest release the service found itself (the About button
    /// has only the version it shows). With nothing found yet (after a restart) it checks first
    /// and installs a newer release if there is one.
    pub fn install_ready(&self) {
        let _ = self.commands.send(Command::InstallReady);
    }

    /// Turns the background check on or off.
    pub fn enable(&self, on: bool) {
        let _ = self.commands.send(Command::Enable(on));
    }

    #[cfg(test)]
    /// Stops the thread and waits for it; a request in flight finishes first.
    pub fn stop(mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for UpdateService {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
    }
}
