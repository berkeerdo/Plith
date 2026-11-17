//! The supervisor thread: launches the helper, watches it and the link once a second, and runs
//! the flyout suppression the mode and the link allow. It never posts to the UI thread.

use super::flyout::{Suppression, mode_name};
use super::health::{Failure, Health, HelperState, Notes, failure_line, give_up_line};
use crate::log::LogFile;
use plith_core::launch::FlyoutMode;
use plith_core::osd::policy::wants_suppression;
use plith_ipc::{HelperProcess, INSTALLED_HELPER, OsdLink, launch_helper};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};

/// How often the supervisor looks at the helper and the link.
pub const TICK: Duration = Duration::from_secs(1);

/// The supervisor thread's name.
const THREAD_NAME: &str = "plith-osd-supervisor";

/// COM on the supervisor thread, apartment threaded, for `launch_helper`'s shell call.
struct Com {
    initialized: bool,
}

impl Com {
    fn init(log: &LogFile) -> Self {
        // SAFETY: plain call on this thread; balanced in `Drop` when it succeeded (S_FALSE
        // included, which also needs a matching uninitialize).
        let result =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        if result.is_err() {
            log.line(&format!("osd: com not initialized: {result:?}"));
        }
        Self {
            initialized: result.is_ok(),
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: balances the successful `CoInitializeEx` in `init`, on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// Everything the supervisor thread owns.
struct Supervisor {
    mode: FlyoutMode,
    link: Arc<OsdLink>,
    log: Arc<LogFile>,
    health: Health,
    helper: Option<HelperProcess>,
    suppression: Suppression,
    notes: Notes,
}

impl Supervisor {
    /// One look at the helper and the link at `now`.
    fn tick(&mut self, now: Instant) {
        let connected = self.link.is_connected();
        let helper = match &self.helper {
            None => HelperState::Absent,
            Some(helper) if helper.has_exited() => HelperState::Exited,
            Some(_) => HelperState::Alive,
        };
        let step = self.health.tick(now, connected, helper);
        match step.link {
            Some(true) => {
                self.notes.clear();
                self.log.line("osd: link connected");
            }
            Some(false) => {
                let error = self.link.last_error();
                let error = error.as_deref().unwrap_or("no error reported");
                self.log.line(&format!("osd: link lost ({error})"));
            }
            None => {}
        }
        if let Some(failure) = step.failure {
            // Let the handle go; the helper is not stopped (it exits on its own).
            self.helper = None;
            self.note(&failure_line(failure, self.link.last_error().as_deref()));
        }
        if step.gave_up {
            self.log.line(&give_up_line());
        }
        if step.launch {
            self.launch(now);
        }
        let wanted = wants_suppression(self.mode, self.health.link_up());
        self.suppression.apply(wanted, &self.log);
        self.suppression.tick(&self.log);
    }

    /// Starts the installed helper and reports the result to the health state.
    fn launch(&mut self, now: Instant) {
        let exe = Path::new(INSTALLED_HELPER);
        let result = if exe.is_file() {
            launch_helper(exe)
        } else {
            Err(format!("helper not installed at {INSTALLED_HELPER}"))
        };
        let ok = result.is_ok();
        match result {
            Ok(helper) => {
                self.log
                    .line(&format!("osd: helper launched (pid {})", helper.pid()));
                self.helper = Some(helper);
            }
            Err(error) => self.note(&failure_line(Failure::Launch, Some(&error))),
        }
        if self.health.launched(now, ok) {
            self.log.line(&give_up_line());
        }
    }

    /// Logs `text` unless it was logged since the link was last up.
    fn note(&mut self, text: &str) {
        if self.notes.fresh(text) {
            self.log.line(text);
        }
    }

    /// Stops the suppression (shows every hidden flyout) and the key hook, then lets the
    /// helper handle go without stopping the helper. The link itself is dropped by the owner.
    fn shutdown(&mut self) {
        self.suppression.stop(&self.log);
        self.helper = None;
    }
}

/// Starts the supervisor thread. It ticks every [`TICK`] until `stop` receives a message or
/// disconnects, then shuts down and returns.
///
/// # Errors
/// The thread could not be spawned.
pub fn spawn(
    mode: FlyoutMode,
    link: Arc<OsdLink>,
    log: Arc<LogFile>,
    stop: Receiver<()>,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name(THREAD_NAME.to_string())
        .spawn(move || run(mode, link, log, &stop))
}

/// The thread body: COM first, the tick loop, the shutdown, COM last.
fn run(mode: FlyoutMode, link: Arc<OsdLink>, log: Arc<LogFile>, stop: &Receiver<()>) {
    let com = Com::init(&log);
    if mode != FlyoutMode::Off {
        let wanted = wants_suppression(mode, true).map_or("off", mode_name);
        log.line(&format!("osd: flyout mode {wanted}"));
    }
    let mut supervisor = Supervisor {
        mode,
        link,
        log,
        health: Health::new(Instant::now()),
        helper: None,
        suppression: Suppression::default(),
        notes: Notes::default(),
    };
    loop {
        supervisor.tick(Instant::now());
        match stop.recv_timeout(TICK) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    supervisor.shutdown();
    drop(supervisor);
    drop(com);
}
