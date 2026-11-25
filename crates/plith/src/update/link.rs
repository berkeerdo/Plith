//! The service as the UI thread holds it: events read without blocking, the latest row kept
//! for the About page, a pending notice kept for the tray.

use super::{Hooks, UpdateEvent, UpdateService, UpdateSettings};
use plith_core::setup::Version;
use plith_core::update::{Release, UpdateRow};
use std::sync::mpsc::Receiver;

/// A running service and what the UI has learned from it.
pub struct UpdateLink {
    service: UpdateService,
    feed: Receiver<UpdateEvent>,
    row: UpdateRow,
    notice: Option<Release>,
}

impl UpdateLink {
    /// Starts the service over the real network.
    #[must_use]
    pub fn start(own: Version, settings: UpdateSettings, hooks: Hooks) -> Self {
        let (service, feed) = UpdateService::start(own, settings, hooks);
        Self {
            service,
            feed,
            row: UpdateRow::Current,
            notice: None,
        }
    }

    /// Starts the service over any backend, for tests.
    #[cfg(test)]
    #[must_use]
    pub fn start_over(
        own: Version,
        settings: UpdateSettings,
        hooks: Hooks,
        backend: Box<dyn super::Backend>,
    ) -> Self {
        let (service, feed) = UpdateService::start_over(own, settings, hooks, backend);
        Self {
            service,
            feed,
            row: UpdateRow::Current,
            notice: None,
        }
    }

    /// Takes every queued event, in order, remembering the row and a notice on the way.
    pub fn poll(&mut self) -> Vec<UpdateEvent> {
        let events: Vec<UpdateEvent> = self.feed.try_iter().collect();
        for event in &events {
            match event {
                UpdateEvent::Row(row) => self.row = *row,
                UpdateEvent::Notice(release) => self.notice = Some(release.clone()),
                UpdateEvent::Checked(_) | UpdateEvent::Seen(_) => {}
            }
        }
        events
    }

    /// What the About row shows now.
    #[must_use]
    pub fn row(&self) -> UpdateRow {
        self.row
    }

    /// The release to announce, once.
    pub fn take_notice(&mut self) -> Option<Release> {
        self.notice.take()
    }

    /// Checks now, whatever the setting says.
    pub fn check_now(&self) {
        self.service.check_now();
    }

    /// Downloads and starts the installer of the newest release found.
    pub fn install_ready(&self) {
        self.service.install_ready();
    }

    /// Turns the background check on or off.
    pub fn enable(&self, on: bool) {
        self.service.enable(on);
    }
}
