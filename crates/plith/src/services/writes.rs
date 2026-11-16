//! Hardware writes off the UI thread: the real services may block (a media command up to 2 s,
//! the microphone toggle on its worker), so the UI thread only queues a [`Write`] and one
//! `plith-writes` thread applies them in order. Results come back as ordinary service updates.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// One write to a service.
#[derive(Debug, Clone, PartialEq)]
pub enum Write {
    /// Make the endpoint with this id the default output.
    DefaultEndpoint(String),
    /// Flip the route from `strip` to `bus`.
    Route {
        /// The Voicemeeter strip.
        strip: usize,
        /// The Voicemeeter bus.
        bus: usize,
    },
    /// Mute or unmute the microphone.
    ToggleMic,
    /// Set the monitor brightness in percent.
    Brightness(u8),
    /// Toggle play and pause.
    PlayPause,
    /// Skip to the next track.
    Next,
    /// Skip to the previous track.
    Previous,
    /// Seek to this position.
    Seek(Duration),
}

/// The queue and the thread that applies the writes, in the order they were sent.
pub struct Writer {
    tx: Option<Sender<Write>>,
    stopping: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Writer {
    /// Starts the `plith-writes` thread, which calls `apply` for every write.
    ///
    /// # Errors
    /// Returns the error of a failed thread spawn.
    pub fn spawn(mut apply: impl FnMut(Write) + Send + 'static) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel::<Write>();
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let join = thread::Builder::new()
            .name(String::from("plith-writes"))
            .spawn(move || {
                for write in rx {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    apply(write);
                }
            })?;
        Ok(Self {
            tx: Some(tx),
            stopping,
            join: Some(join),
        })
    }

    /// Queues a write; returns at once. After [`stop`](Self::stop) it is dropped.
    pub fn send(&self, write: Write) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(write);
        }
    }

    /// Drops the writes still queued, waits for the one in flight and joins the thread.
    /// Returns how long the join took (log it with [`stop_line`]); `None` when it was already
    /// stopped, as a repeat call does nothing.
    pub fn stop(&mut self) -> Option<Duration> {
        self.stopping.store(true, Ordering::Release);
        self.tx = None;
        let join = self.join.take()?;
        let begun = Instant::now();
        let _ = join.join();
        Some(begun.elapsed())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// The log line of a writes stop, in the format of the service stop lines.
#[must_use]
pub fn stop_line(took: Duration) -> String {
    format!(
        "service writes: stopped in {:.1} ms",
        took.as_secs_f64() * 1000.0
    )
}

#[cfg(test)]
mod tests;
