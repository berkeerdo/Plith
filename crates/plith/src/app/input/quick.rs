//! The quick page's clicks: a look applies and is saved at once, the settings row opens the
//! settings window.

use crate::app::state::App;
use plith_core::pages::PageId;
use plith_core::settings::IdleLook;
use plith_core::settings::change::SettingChange;

impl App {
    /// A look was picked on the quick page: the same change path as the settings window.
    pub(in crate::app) fn on_look(&mut self, look: IdleLook) {
        self.log.line(&format!("quick: look {}", look.key()));
        self.change(SettingChange::Look(look));
    }

    /// The settings row was clicked: opens the settings window, or raises the open one.
    pub(in crate::app) fn on_open_settings(&mut self) {
        self.log.line("quick: open settings");
        self.open_sheet();
    }

    /// The page an opening with no page of its own lands on. `open_on = LastPage` never records
    /// the quick page: the island then opens on the welcome view.
    pub(in crate::app) fn last_page(&self) -> Option<PageId> {
        let page = self.mailbox.latest().scene.page;
        (page == PageId::Quick.index()).then_some(PageId::Welcome)
    }
}
