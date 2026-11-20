//! Saves the settings file off the UI thread: one worker blocked on a channel (no periodic
//! wake), the newest queued value wins, and the value still pending is written at shutdown.

use crate::log::LogFile;
use crate::settings_file::save_at;
use plith_core::settings::Settings;
use plith_shell::UiWaker;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

/// Called on the worker: wakes the UI thread.
type Wake = Box<dyn Fn() + Send>;
/// Called on the worker with a failure's log line.
type Report = Box<dyn Fn(&str) + Send>;

/// The settings writer of a normal run.
pub struct Saver {
    tx: Option<Sender<Settings>>,
    failed: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Saver {
    /// Starts the worker writing to `path`; a change of the failure flag wakes the UI thread
    /// through `waker`, and a failure is logged to `log`.
    #[must_use]
    pub fn spawn(path: PathBuf, waker: UiWaker, log: Arc<LogFile>) -> Saver {
        Self::start(
            path,
            Box::new(move || waker.wake()),
            Box::new(move |line| log.line(line)),
        )
    }

    /// As [`Self::spawn`], with plain callbacks. If the thread cannot start, nothing is saved
    /// for the run and [`Self::failed`] says so from the start.
    #[must_use]
    pub fn start(path: PathBuf, wake: Wake, report: Report) -> Saver {
        let (tx, rx) = mpsc::channel();
        let failed = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            path,
            failed: Arc::clone(&failed),
            wake,
            report,
        };
        let join = std::thread::Builder::new()
            .name(String::from("plith-settings-save"))
            .spawn(move || worker.run(&rx))
            .ok();
        if join.is_none() {
            failed.store(true, Ordering::Release);
        }
        Saver {
            tx: join.is_some().then_some(tx),
            failed,
            join,
        }
    }

    /// Queues `settings` for writing; a newer value queued before the worker runs replaces it.
    pub fn save(&self, settings: Settings) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(settings);
        }
    }

    /// Whether the last write failed.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    /// Writes the value still queued, then ends the worker and waits for it.
    pub fn flush_and_join(mut self) {
        drop(self.tx.take());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The worker thread's state.
struct Worker {
    path: PathBuf,
    failed: Arc<AtomicBool>,
    wake: Wake,
    report: Report,
}

impl Worker {
    /// Writes the newest queued value until the sender is gone; the queue drains first.
    fn run(&self, rx: &Receiver<Settings>) {
        while let Ok(first) = rx.recv() {
            let newest = rx.try_iter().last().unwrap_or(first);
            self.write(&newest);
        }
    }

    fn write(&self, settings: &Settings) {
        let result = save_at(&self.path, settings);
        if let Err(error) = &result {
            (self.report)(&format!("settings: cannot save the file: {error}"));
        }
        let failed = result.is_err();
        if self.failed.swap(failed, Ordering::AcqRel) != failed {
            (self.wake)();
        }
    }
}

#[cfg(test)]
mod tests;
