use super::*;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_host::autostart::Autostart;
use plith_shell::ShellEvent;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Default)]
struct CountQuit(Rc<Cell<u32>>);

impl Quit for CountQuit {
    fn quit(&self) {
        self.0.set(self.0.get() + 1);
    }
}

fn dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plith-tray-{name}-{}", std::process::id()))
}

fn app(name: &str) -> App {
    let log = plith_host::log::LogFile::open_at(&dir(name).join("plith.log")).unwrap();
    App::new(
        Arc::new(SceneMailbox::new(Scene::default(), Arc::default())),
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    )
}

fn log_of(app: &App, name: &str) -> String {
    app.log.flush();
    std::fs::read_to_string(dir(name).join("plith.log")).unwrap_or_default()
}

#[test]
fn the_quit_item_closes_the_app_once_and_logs_it() {
    let mut app = app("quit");
    let quit = CountQuit::default();
    app.attach_tray(None, quit.clone());
    app.on_event(ShellEvent::Tray(TrayItem::Quit));
    assert_eq!(quit.0.get(), 1);
    assert!(log_of(&app, "quit").contains("tray: quit"));
}

#[test]
fn the_settings_item_does_not_quit_and_takes_the_quick_pages_path() {
    let mut app = app("settings");
    let quit = CountQuit::default();
    app.attach_tray(None, quit.clone());
    app.on_event(ShellEvent::Tray(TrayItem::Settings));
    assert_eq!(quit.0.get(), 0);
    let log = log_of(&app, "settings");
    assert!(log.contains("tray: open settings"), "{log}");
    // The test app has no window attached, so the open goes no further.
    assert!(log.contains("settings window: not attached"), "{log}");
}

#[test]
fn a_tray_command_before_the_tray_is_wired_does_nothing_and_does_not_panic() {
    let mut app = app("unwired");
    app.on_event(ShellEvent::Tray(TrayItem::Quit));
    app.sync_tray(Lang::Tr);
}

/// A switch that remembers what it was told.
struct FakeAutostart {
    on: Rc<Cell<bool>>,
    fail: bool,
}

impl Autostart for FakeAutostart {
    fn is_on(&self) -> bool {
        self.on.get()
    }

    fn set(&mut self, on: bool) -> Result<(), String> {
        if self.fail {
            return Err(String::from("denied"));
        }
        self.on.set(on);
        Ok(())
    }
}

#[test]
fn the_genel_switch_writes_and_the_row_follows_what_the_registry_holds() {
    let mut app = app("switch");
    let on = Rc::new(Cell::new(false));
    app.attach_autostart(Box::new(FakeAutostart {
        on: Rc::clone(&on),
        fail: false,
    }));
    assert!(!app.sheet_facts().autostart);
    app.set_autostart(true);
    assert!(on.get());
    assert!(app.sheet_facts().autostart);
    app.set_autostart(false);
    assert!(!on.get());
    assert!(!app.sheet_facts().autostart);
    let log = log_of(&app, "switch");
    assert!(
        log.contains("autostart: on") && log.contains("autostart: off"),
        "{log}"
    );
}

#[test]
fn a_failed_write_is_logged_and_the_row_springs_back() {
    let mut app = app("fail");
    let on = Rc::new(Cell::new(false));
    app.attach_autostart(Box::new(FakeAutostart {
        on: Rc::clone(&on),
        fail: true,
    }));
    app.set_autostart(true);
    assert!(!app.sheet_facts().autostart, "the registry still says off");
    assert!(log_of(&app, "fail").contains("autostart: cannot switch: denied"));
}

#[test]
fn opening_the_window_reads_the_registry_again() {
    let mut app = app("reread");
    let on = Rc::new(Cell::new(true));
    app.attach_autostart(Box::new(FakeAutostart {
        on: Rc::clone(&on),
        fail: false,
    }));
    // Not attached: the open stops early but has read the switch already.
    app.open_sheet();
    assert!(
        app.sheet_facts().autostart,
        "on at start, as Windows had it"
    );
}
