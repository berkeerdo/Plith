//! The update service as the app sees it: attached in a normal run, drained on a wake. The
//! service never writes a file; the two values it learns (the last check, the last noticed
//! version) are saved here, through the settings saver, on the UI thread.

use super::App;
use crate::update::{UpdateEvent, UpdateLink};

impl App {
    /// Keeps the running update service; a normal run only.
    pub fn attach_update(&mut self, link: UpdateLink) {
        self.update = Some(link);
    }

    /// The update service, for the settings window and the tray.
    #[allow(dead_code, reason = "the settings rows of the next task read the link")]
    pub fn update(&mut self) -> Option<&mut UpdateLink> {
        self.update.as_mut()
    }

    /// On a wake: takes the service's events and saves what they record.
    pub(super) fn drain_update(&mut self) {
        let Some(link) = &mut self.update else {
            return;
        };
        let mut changed = false;
        for event in link.poll() {
            match event {
                UpdateEvent::Checked(at) if self.settings.update_checked != at => {
                    self.settings.update_checked = at;
                    changed = true;
                }
                UpdateEvent::Seen(version) => {
                    let seen = version.to_string();
                    if self.settings.update_seen != seen {
                        self.settings.update_seen = seen;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        if changed {
            self.queue_save(self.settings.clone());
        }
    }
}
