//! The thread that starts the elevated worker and carries what it reports to the window.
//!
//! The Windows prompt blocks `ShellExecuteExW` for as long as the user takes, so it must not
//! run on the UI thread (the window would stop answering). Everything reaches the UI through an
//! inbox the window drains when it is woken.

use crate::elevate::{ElevateError, start_worker};
use crate::pipe::{ReportServer, report_pipe_name};
use plith_core::setup::{Job, Report};
use plith_shell::setup_window::wake;
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};

/// How long one wait for a report lasts before the worker's process is looked at.
const POLL: Duration = Duration::from_millis(250);
/// How long the pipe is given to deliver what the worker sent just before it exited.
const DRAIN: Duration = Duration::from_millis(300);
/// The pause when the pipe is closed but the process has not been seen to end yet.
const PAUSE: Duration = Duration::from_millis(100);

/// What the pump tells the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// A report from the worker.
    Report(Report),
    /// The worker ended; its exit code. Sent only when it ended without a `Finished` or
    /// `Failed` report.
    Exit(u32),
    /// The worker could not be started.
    ElevateFailed {
        /// The user declined the Windows prompt.
        cancelled: bool,
        /// What happened, in English, for the log.
        text: String,
    },
    /// The renderer died; the window can no longer show anything.
    RenderFatal(String),
}

/// What the pump thread and the UI share.
pub type Inbox = Arc<Mutex<VecDeque<Msg>>>;

/// Puts `msg` in the inbox and wakes the window.
pub fn send(inbox: &Inbox, hwnd: isize, msg: Msg) {
    inbox
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push_back(msg);
    wake(hwnd);
}

/// Takes everything waiting in the inbox.
pub fn take_all(inbox: &Inbox) -> Vec<Msg> {
    inbox
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain(..)
        .collect()
}

/// Whether the report ends the worker's story.
fn ends(report: &Report) -> bool {
    matches!(report, Report::Finished | Report::Failed { .. })
}

/// Starts the pump for `job`: it makes the pipe, starts the worker, forwards its reports and
/// ends with the last report or, failing that, the worker's exit code.
///
/// # Errors
/// The thread could not be started.
pub fn spawn(job: Job, hwnd: isize, inbox: Inbox) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name(String::from("plith-setup-pump"))
        .spawn(move || {
            // The shell wants an apartment for the prompt; a failure only means it has one.
            // SAFETY: balanced by the `CoUninitialize` below on this same thread when it succeeded.
            let com =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
                    .is_ok();
            pump(job, hwnd, &inbox);
            if com {
                // SAFETY: balances the successful `CoInitializeEx` above.
                unsafe { CoUninitialize() };
            }
        })
}

fn pump(job: Job, hwnd: isize, inbox: &Inbox) {
    let say = |msg| send(inbox, hwnd, msg);
    let name = report_pipe_name();
    let server = match ReportServer::bind(&name) {
        Ok(server) => server,
        Err(error) => {
            return say(Msg::ElevateFailed {
                cancelled: false,
                text: format!("report pipe: {error}"),
            });
        }
    };
    let worker = match start_worker(job, &name, hwnd) {
        Ok(worker) => worker,
        Err(error) => {
            return say(Msg::ElevateFailed {
                cancelled: error == ElevateError::Cancelled,
                text: error.to_string(),
            });
        }
    };
    loop {
        if let Some(report) = server.next(POLL) {
            let last = ends(&report);
            say(Msg::Report(report));
            if last {
                return;
            }
            continue;
        }
        let Some(code) = worker.exit_code() else {
            // Nothing came and the worker lives; a closed pipe returns at once, so rest.
            thread::sleep(PAUSE);
            continue;
        };
        // The worker may have written its last reports just before it ended.
        while let Some(report) = server.next(DRAIN) {
            let last = ends(&report);
            say(Msg::Report(report));
            if last {
                return;
            }
        }
        return say(Msg::Exit(code));
    }
}
