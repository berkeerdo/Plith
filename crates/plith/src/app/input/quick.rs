//! The quick page's clicks. Task 10 wires the settings window; until then both only log.

use crate::app::state::App;
use plith_core::pages::PageId;
use plith_core::settings::IdleLook;

impl App {
    /// A look was picked on the quick page. It will apply the look and save it; for now it logs.
    pub(in crate::app) fn on_look(&mut self, look: IdleLook) {
        self.log.line(&format!("quick: look {}", look.key()));
    }

    /// The settings row was clicked. It will open the settings window; for now it logs.
    pub(in crate::app) fn on_open_settings(&mut self) {
        self.log.line("quick: open settings");
    }

    /// The page an opening with no page of its own lands on. `open_on = LastPage` never records
    /// the quick page: the island then opens on the welcome view.
    pub(in crate::app) fn last_page(&self) -> Option<PageId> {
        let page = self.mailbox.latest().scene.page;
        (page == PageId::Quick.index()).then_some(PageId::Welcome)
    }
}
