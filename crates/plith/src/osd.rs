//! The notch side of the OSD: the link to the UIAccess helper, the supervisor thread that starts
//! and watches the helper, the flyout suppression it runs, and the sink the feed sends to.
//!
//! The feed's sink holds only a `Weak` link, so dropping [`OsdSide`] drops the link at a known
//! point (after the supervisor stopped and restored any hidden flyout), which sends `Bye`.

mod flyout;
mod health;
mod supervisor;

pub use flyout::restore_now;

use crate::log::LogFile;
use plith_core::launch::FlyoutMode;
use plith_core::osd::{OsdLevel, OsdMeter};
use plith_ipc::security::own_integrity_above_medium;
use plith_ipc::{Message, Meter, OsdLink, current_pipe_name};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Receives every level the OSD should show.
pub type OsdSink = Box<dyn FnMut(OsdLevel)>;

/// The line written when the notch runs elevated and the OSD is not started.
pub const ELEVATED_LINE: &str = "osd: plith runs above medium integrity; the installed helper \
                                 would not be trusted, so the OSD is not started";

/// How long exit waits for the supervisor before leaving it behind (the UI thread waits).
pub const STOP_WAIT: Duration = Duration::from_millis(1500);

/// How often [`join_within`] looks at the thread.
const STOP_POLL: Duration = Duration::from_millis(10);

/// Joins `thread` if it finishes within `timeout`; `None` (the thread detached) otherwise.
pub(crate) fn join_within(thread: JoinHandle<()>, timeout: Duration) -> Option<thread::Result<()>> {
    let deadline = Instant::now() + timeout;
    while !thread.is_finished() {
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(STOP_POLL);
    }
    Some(thread.join())
}

/// Pure. The wire message for one level.
#[must_use]
pub fn level_message(level: OsdLevel) -> Message {
    Message::Level {
        meter: match level.meter {
            OsdMeter::Volume => Meter::Volume,
            OsdMeter::Brightness => Meter::Brightness,
        },
        percent: level.percent,
        muted: level.muted,
        label: level.label,
        muted_word: level.muted_word,
    }
}

/// The OSD side of the session: the link and its supervisor. Dropping it stops the supervisor
/// (suppression off, hidden flyouts shown, key hook off), then drops the link, which sends
/// `Bye`; the helper fades out and exits by itself.
pub struct OsdSide {
    link: Option<Arc<OsdLink>>,
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
    log: Arc<LogFile>,
}

impl OsdSide {
    /// Starts the link and the supervisor. `None` (with one log line) when the process runs
    /// above medium integrity, where the link would fail closed, or when the thread cannot
    /// be spawned.
    #[must_use]
    pub fn start(mode: FlyoutMode, log: Arc<LogFile>) -> Option<Self> {
        match own_integrity_above_medium() {
            Ok(true) => {
                log.line(ELEVATED_LINE);
                return None;
            }
            Ok(false) => {}
            Err(error) => log.line(&format!("osd: own integrity not read ({error})")),
        }
        let link = Arc::new(OsdLink::start(current_pipe_name()));
        let (stop, stopped) = mpsc::channel();
        match supervisor::spawn(mode, Arc::clone(&link), Arc::clone(&log), stopped) {
            Ok(thread) => Some(Self {
                link: Some(link),
                stop: Some(stop),
                thread: Some(thread),
                log,
            }),
            Err(error) => {
                log.line(&format!("osd: supervisor not started: {error}"));
                None
            }
        }
    }

    /// A sink for the feed: maps each level to `Message::Level` and sends it on the link
    /// without blocking. Holds the link weakly; after drop it does nothing.
    #[must_use]
    pub fn sink(&self) -> OsdSink {
        let link: Weak<OsdLink> = self.link.as_ref().map_or_else(Weak::new, Arc::downgrade);
        let log = Arc::clone(&self.log);
        Box::new(move |level| {
            let Some(link) = link.upgrade() else {
                return;
            };
            if let Err(error) = link.send(level_message(level)) {
                log.line(&format!("osd: level not sent: {error:?}"));
            }
        })
    }
}

impl Drop for OsdSide {
    fn drop(&mut self) {
        // Disconnecting the channel wakes the supervisor's wait at once.
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            match join_within(thread, STOP_WAIT) {
                Some(Ok(())) => {}
                Some(Err(_)) => self.log.line("osd: supervisor panicked"),
                None => {
                    // Stuck in a shell launch: left behind. It still stops suppression and
                    // drops its link reference when the call returns; show hidden flyouts now.
                    self.log.line("osd: supervisor did not stop in time");
                    restore_now();
                }
            }
        }
        // Normally the last strong reference: sends `Bye` and returns within about 1 s. A
        // supervisor left behind holds the last one, and `Bye` goes when it ends.
        drop(self.link.take());
    }
}

#[cfg(test)]
mod tests;
