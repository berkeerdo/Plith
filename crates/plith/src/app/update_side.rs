//! Starts the update service of a normal run. Probing runs (demo, idle check, fixture) never
//! touch the network.

use super::idle_wiring;
use super::state::App;
use crate::update::{Hooks, UpdateLink, UpdateSettings};
use plith_core::launch::Launch;
use plith_core::settings::Settings;
use plith_core::setup::Version;
use plith_host::log::LogFile;
use plith_shell::Shell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Starts the service from `settings` and hands it to the app. It wakes the UI thread only to
/// deliver an event, so a resting app stays at zero wakes.
pub fn start(
    launch: &Launch,
    log: &Arc<LogFile>,
    shell: &Shell,
    app: &Rc<RefCell<App>>,
    settings: &Settings,
) {
    if !idle_wiring::is_normal(launch) {
        return;
    }
    let Some(own) = Version::parse(env!("CARGO_PKG_VERSION")) else {
        log.line("update: own version unreadable, check off");
        return;
    };
    let waker = shell.ui_waker();
    let failures = Arc::clone(log);
    let hooks = Hooks {
        waker: Box::new(move || waker.wake()),
        on_failure: Box::new(move |line| failures.line(line)),
    };
    let from = UpdateSettings {
        enabled: settings.update_check,
        checked: settings.update_checked,
        seen: settings.update_seen.clone(),
    };
    log.line(&format!(
        "update: {}",
        if from.enabled { "on" } else { "off" }
    ));
    app.borrow_mut()
        .attach_update(UpdateLink::start(own, from, hooks));
}
