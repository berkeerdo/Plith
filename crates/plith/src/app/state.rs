//! The UI-thread state of the app: one [`App`] reached by the shell handler and the panic hook.

use super::feed::Feed;
use super::wire::raised_line;
use crate::log::LogFile;
use plith_core::scene::{Scene, SceneMailbox};
use plith_render::StopHandle;
use plith_shell::ShellEvent;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Something that stops and joins the render thread. A trait so tests can use a fake.
pub trait Stopper {
    /// Asks the render thread to end and waits for it.
    fn stop_and_join(&self);
}

impl Stopper for StopHandle {
    fn stop_and_join(&self) {
        StopHandle::stop_and_join(self);
    }
}

/// Everything the UI thread owns. It lives in an `Rc<RefCell<App>>` shared by the shell handler
/// and the panic hook; both run on the UI thread only.
///
/// Re-entrancy: the shell may deliver an event while an earlier `emit` is draining. Never hold
/// the `borrow_mut` across a call that can pump messages or re-enter `emit`. `Closing` joins the
/// render thread, which never sends to the UI thread, so that path does not re-enter.
pub struct App {
    /// The scene the render thread reads.
    pub mailbox: Arc<SceneMailbox>,
    /// The run log.
    pub log: Arc<LogFile>,
    /// Set on `Closing`; the render sinks read it from another thread, so it stays an `Arc`.
    pub closing: Arc<AtomicBool>,
    stop: Option<Box<dyn Stopper>>,
    feed: Option<Feed>,
}

impl App {
    /// A new app with no render thread attached yet.
    #[must_use]
    pub fn new(mailbox: Arc<SceneMailbox>, log: Arc<LogFile>, closing: Arc<AtomicBool>) -> Self {
        Self {
            mailbox,
            log,
            closing,
            stop: None,
            feed: None,
        }
    }

    /// Starts feeding the pages from the services; until then a wake or a timer does nothing.
    pub fn attach_feed(&mut self, feed: Feed) {
        self.feed = Some(feed);
    }

    /// Lets the feed react to a scene change.
    fn scene_changed(&mut self) {
        if let Some(feed) = &mut self.feed {
            feed.on_scene(&self.mailbox);
        }
    }

    /// Records how to stop the render thread, once it has been spawned.
    pub fn attach_render(&mut self, stop: impl Stopper + 'static) {
        self.stop = Some(Box::new(stop));
    }

    /// Stops and joins the render thread under the close backstop, if one is attached.
    pub fn close_render(&self) {
        if let Some(stop) = &self.stop {
            super::backstop::close_render(&self.log, stop.as_ref());
        }
    }

    /// Handles one shell event. On `Closing` (`WM_CLOSE`, before the window is destroyed) it sets
    /// `closing`, then stops and joins the render thread, which presents to the window about to
    /// be destroyed.
    pub fn on_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::Closing => {
                self.closing.store(true, Ordering::Release);
                self.close_render();
            }
            ShellEvent::Click => {
                let _ = self.mailbox.update(|scene| Scene {
                    expanded: !scene.expanded,
                    ..scene
                });
                self.scene_changed();
            }
            ShellEvent::Suppression(suppression) => {
                let _ = self.mailbox.update(|scene| Scene {
                    suppression,
                    ..scene
                });
                self.scene_changed();
                self.log.line(&format!(
                    "suppression: display_off={} locked={} fullscreen={}",
                    suppression.display_off, suppression.locked, suppression.fullscreen
                ));
            }
            ShellEvent::Wake => {
                if let Some(feed) = &mut self.feed {
                    feed.on_wake(&self.mailbox);
                }
            }
            ShellEvent::Timer(id) => {
                if let Some(feed) = &mut self.feed {
                    feed.on_timer(id, &self.mailbox);
                }
            }
            ShellEvent::DisplayChanged => self.log.line("display changed: restart needed"),
            ShellEvent::Raised {
                foreground,
                above,
                immediate,
            } => self
                .log
                .line(&raised_line(&foreground, above.as_deref(), immediate)),
        }
    }
}

#[cfg(test)]
mod tests;
