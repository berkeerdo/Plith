//! Starts the tray icon and the start with Windows switch of a normal run.

use super::idle_wiring::{self, language_of};
use super::state::App;
use plith_core::launch::Launch;
use plith_core::settings::Settings;
use plith_core::tray::menu::menu;
use plith_host::autostart::RegistryAutostart;
use plith_host::log::LogFile;
use plith_shell::Shell;
use std::cell::RefCell;
use std::rc::Rc;

/// Puts the icon in the notification area and gives the settings window its Windows start
/// switch. Probing runs (demo, idle check, fixture) stay without both: a measurement must not
/// leave an icon behind or read the user's registry.
pub fn start(
    launch: &Launch,
    log: &LogFile,
    shell: &Shell,
    app: &Rc<RefCell<App>>,
    settings: &Settings,
) {
    if !idle_wiring::is_normal(launch) {
        return;
    }
    app.borrow_mut()
        .attach_tray(Some(shell.tray_handle()), shell.quit_handle());
    match shell.add_tray(&menu(language_of(settings))) {
        Ok(()) => log.line("tray: icon added"),
        // At sign in Explorer may not have its taskbar yet; `TaskbarCreated` adds the icon then.
        Err(error) => log.line(&format!("tray: icon not added yet: {error}")),
    }
    match RegistryAutostart::for_running_exe() {
        Ok(autostart) => app.borrow_mut().attach_autostart(Box::new(autostart)),
        Err(error) => log.line(&format!("autostart: unavailable: {error}")),
    }
}
