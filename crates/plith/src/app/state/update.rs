//! The update service as the app sees it: attached in a normal run, drained on a wake. The
//! service never writes a file; the two values it learns (the last check, the last noticed
//! version) are saved here, through the settings saver, on the UI thread. The row of the About
//! section follows the service's events, and a new version is announced once in the tray.

use super::App;
use crate::update::{UpdateEvent, UpdateLink};
use plith_core::setup::Version;
use plith_core::update::UpdateRow;

impl App {
    /// Keeps the running update service; a normal run only.
    pub fn attach_update(&mut self, link: UpdateLink) {
        self.update = Some(link);
    }

    /// What the About update row shows: the service's last row, or the up to date state when
    /// there is no service (a probing run).
    pub(super) fn update_row(&self) -> UpdateRow {
        self.update
            .as_ref()
            .map_or(UpdateRow::Current, UpdateLink::row)
    }

    /// The About `DENETLE` button.
    pub(super) fn check_update_now(&self) {
        if let Some(link) = &self.update {
            self.log.line("update: check requested");
            link.check_now();
        }
    }

    /// The About `KUR` button and a click on the tray notice: downloads and starts the
    /// installer of the newest release the service found.
    pub(super) fn install_update(&self) {
        if let Some(link) = &self.update {
            self.log.line("update: install requested");
            link.install_ready();
        }
    }

    /// On a wake: takes the service's events, saves what they record, redraws the About row and
    /// shows the notice.
    pub(super) fn drain_update(&mut self) {
        let Some(link) = &mut self.update else {
            return;
        };
        let events = link.poll();
        let notice = link.take_notice();
        let mut changed = false;
        let mut row_moved = false;
        for event in events {
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
                UpdateEvent::Row(_) => row_moved = true,
                _ => {}
            }
        }
        if changed {
            self.queue_save(self.settings.clone());
        }
        if let Some(release) = notice {
            self.announce_update(release.version);
        }
        if row_moved {
            self.refresh_sheet();
        }
    }

    /// The tray balloon for `version`. The service already held the notice back while the user
    /// was busy and records `Seen` only when it sends one, so the app shows what it gets.
    fn announce_update(&self, version: Version) {
        let lang = self.sheet_facts().lang;
        if self.tray.show_notice(
            &plith_core::tray::notice::title(version, lang),
            plith_core::tray::notice::body(lang),
        ) {
            self.log
                .line(&format!("update: notice shown for {version}"));
        } else {
            self.log.line("update: notice skipped: no tray icon");
        }
    }

    /// The balloon was clicked: install, as the About `KUR` button does.
    pub(super) fn on_tray_notice(&self) {
        self.log.line("tray: notice clicked");
        self.install_update();
    }
}

#[cfg(test)]
mod tests;
