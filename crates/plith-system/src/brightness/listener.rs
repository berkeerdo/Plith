//! The `WmiMonitorBrightnessEvent` listener thread and the state it shares with the service.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use super::panel_event::on_panel_event;
use super::service::{ComGuard, Msg};
use super::wmi;
use super::wql::panel_id;
use crate::flyout::SuppressionWindow;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the listener thread shares with the handle: the suppression window the consumer wired
/// and the text of a start failure.
#[derive(Default)]
pub(super) struct ListenerState {
    window: Mutex<Option<Arc<SuppressionWindow>>>,
    error: Mutex<Option<String>>,
}

impl ListenerState {
    pub(super) fn set_window(&self, window: Option<Arc<SuppressionWindow>>) {
        *locked(&self.window) = window;
    }

    pub(super) fn error(&self) -> Option<String> {
        locked(&self.error).clone()
    }

    pub(super) fn set_error(&self, text: Option<String>) {
        *locked(&self.error) = text;
    }

    /// Clones the window out so the event handler never holds the lock while it works.
    fn window(&self) -> Option<Arc<SuppressionWindow>> {
        locked(&self.window).clone()
    }
}

/// The event listener thread, stopped and joined on drop.
pub(super) struct Listener {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Starts the thread. A failure to start listening (COM or WMI) goes to `on_error` once;
    /// each event opens the suppression window, then sends the new level on `tx`.
    pub(super) fn spawn(
        tx: Sender<Msg>,
        state: Arc<ListenerState>,
        on_error: impl FnOnce(String) + Send + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("plith-brightness-events".into())
            .spawn(move || {
                let _com = match ComGuard::init() {
                    Ok(guard) => guard,
                    Err(err) => return on_error(format!("COM initialization failed: {err}")),
                };
                let result = wmi::listen(&flag, |instance, percent| {
                    let window = state.window();
                    on_panel_event(window.as_deref(), Instant::now(), || {
                        let _ = tx.send(Msg::Event {
                            id: panel_id(&instance),
                            percent,
                        });
                    });
                });
                if let Err(text) = result {
                    on_error(text);
                }
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
