use super::*;
use crate::app::state::tray::Balloon;
use crate::update::{Backend, Hooks, LaunchError, UpdateSettings};
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_core::settings::LangChoice;
use plith_shell::ShellEvent;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

const OWN: Version = Version::new(0, 4, 0);
const NEWER: Version = Version::new(0, 5, 0);

fn release_json(tag: &str) -> String {
    let name = format!("Plith-Setup-{}.exe", tag.trim_start_matches('v'));
    let base = "https://github.com/berkeerdo/Plith/releases";
    format!(
        r#"{{"tag_name":"{tag}","html_url":"{base}/tag/{tag}","assets":[{{"name":"{name}","browser_download_url":"{base}/download/{tag}/{name}"}}]}}"#
    )
}

/// A network that always has v0.5.0 and counts what the service asks of it.
#[derive(Default)]
struct Counts {
    fetches: AtomicUsize,
    launches: AtomicUsize,
}

struct Net {
    counts: Arc<Counts>,
    dir: PathBuf,
}

impl Backend for Net {
    fn latest(&self) -> Result<Option<String>, String> {
        self.counts.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(Some(release_json("v0.5.0")))
    }

    fn download(&self, _: &str, dest: &Path, _: &mut dyn FnMut(u8)) -> Result<(), String> {
        std::fs::write(dest, b"setup").map_err(|e| e.to_string())
    }

    fn launch(&self, _: &Path, _: Version) -> Result<(), LaunchError> {
        self.counts.launches.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn busy(&self) -> bool {
        false
    }

    fn now(&self) -> u64 {
        1_000_000
    }

    fn updates_dir(&self) -> Result<PathBuf, String> {
        Ok(self.dir.clone())
    }
}

/// A balloon that records what it was asked to show; `sent` is what it reports (false: no icon).
#[derive(Clone)]
struct Shown {
    shown: Rc<RefCell<Vec<(String, String)>>>,
    sent: bool,
}

impl Balloon for Shown {
    fn show(&self, title: &str, body: &str) -> bool {
        self.shown
            .borrow_mut()
            .push((title.to_owned(), body.to_owned()));
        self.sent
    }
}

fn dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("plith-appupdate-{name}-{}", std::process::id()))
}

fn app(name: &str) -> App {
    let log = plith_host::log::LogFile::open_at(&dir(name).join("plith.log")).unwrap();
    let mut app = App::new(
        Arc::new(SceneMailbox::new(Scene::default(), Arc::default())),
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    );
    app.settings.lang = LangChoice::Code("tr".into());
    app
}

/// An app with a service over [`Net`] (the background check off: only commands run) and a
/// balloon that reports `sent`.
fn wired(name: &str, sent: bool) -> (App, Shown, Arc<Counts>) {
    let mut app = app(name);
    let counts = Arc::new(Counts::default());
    let net = Net {
        counts: Arc::clone(&counts),
        dir: dir(name).join("updates"),
    };
    let hooks = Hooks {
        waker: Box::new(|| {}),
        on_failure: Box::new(|_| {}),
    };
    let from = UpdateSettings {
        enabled: false,
        checked: 0,
        seen: String::new(),
    };
    app.attach_update(UpdateLink::start_over(OWN, from, hooks, Box::new(net)));
    let balloon = Shown {
        shown: Rc::default(),
        sent,
    };
    app.attach_notice(balloon.clone());
    (app, balloon, counts)
}

/// Drains the service's events on the app until `done` holds.
fn settle(app: &mut App, done: impl Fn(&App) -> bool) {
    for _ in 0..500 {
        app.drain_update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the update service did not reach the state in time");
}

fn ready(app: &App) -> bool {
    app.update_row() == UpdateRow::Ready(NEWER)
}

fn log_of(app: &App, name: &str) -> String {
    app.log.flush();
    std::fs::read_to_string(dir(name).join("plith.log")).unwrap_or_default()
}

#[test]
fn without_a_service_the_row_claims_nothing() {
    let app = app("none");
    assert_eq!(app.update_row(), UpdateRow::Unknown);
    assert_eq!(app.sheet_facts().update, UpdateRow::Unknown);
    app.check_update_now();
    app.install_update();
}

#[test]
fn the_row_follows_the_service_into_the_sheet_facts() {
    let (mut app, _, counts) = wired("row", true);
    app.check_update_now();
    settle(&mut app, ready);
    assert_eq!(app.sheet_facts().update, UpdateRow::Ready(NEWER));
    assert_eq!(counts.fetches.load(Ordering::SeqCst), 1);
}

#[test]
fn a_new_version_shows_one_balloon_in_the_users_language() {
    let (mut app, balloon, counts) = wired("balloon", true);
    app.check_update_now();
    settle(&mut app, ready);
    assert_eq!(
        *balloon.shown.borrow(),
        vec![(
            "Plith 0.5.0 haz\u{131}r".to_owned(),
            "Kurmak i\u{e7}in t\u{131}kla".to_owned()
        )]
    );
    // The same version found again is not announced again.
    app.check_update_now();
    settle(&mut app, |_| counts.fetches.load(Ordering::SeqCst) == 2);
    app.drain_update();
    assert_eq!(balloon.shown.borrow().len(), 1);
    assert!(log_of(&app, "balloon").contains("update: notice shown for 0.5.0"));
}

#[test]
fn a_balloon_that_was_not_sent_is_logged_as_skipped() {
    let (mut app, balloon, _) = wired("skipped", false);
    app.check_update_now();
    settle(&mut app, ready);
    assert_eq!(balloon.shown.borrow().len(), 1, "the app tried once");
    let log = log_of(&app, "skipped");
    assert!(log.contains("notice skipped: no tray icon"), "{log}");
    assert!(!log.contains("notice shown"), "{log}");
}

#[test]
fn clicking_the_balloon_installs_the_ready_version_once() {
    let (mut app, _, counts) = wired("click", true);
    app.check_update_now();
    settle(&mut app, ready);
    app.on_event(ShellEvent::TrayNotice);
    settle(&mut app, |_| counts.launches.load(Ordering::SeqCst) == 1);
    assert!(log_of(&app, "click").contains("tray: notice clicked"));
}

#[test]
fn the_about_install_button_starts_the_same_install() {
    let (mut app, _, counts) = wired("install", true);
    app.check_update_now();
    settle(&mut app, ready);
    app.install_update();
    settle(&mut app, |_| counts.launches.load(Ordering::SeqCst) == 1);
}

#[test]
fn a_balloon_click_after_a_restart_checks_then_installs() {
    // After a restart nothing is ready yet; the notice still in the notification centre is the
    // user's intent, so the click checks and, finding the newer release, installs it.
    let (mut app, _, counts) = wired("restart", true);
    app.on_event(ShellEvent::TrayNotice);
    settle(&mut app, |_| counts.launches.load(Ordering::SeqCst) == 1);
    assert_eq!(counts.fetches.load(Ordering::SeqCst), 1);
}

#[test]
fn a_fresh_service_claims_nothing_until_it_checked() {
    let (mut app, _, _) = wired("fresh", true);
    app.drain_update();
    assert_eq!(app.update_row(), UpdateRow::Unknown);
}

#[test]
fn a_row_change_redraws_the_open_sheet() {
    let (mut app, _, _) = wired("redraw", true);
    let before = app.sheet.refreshes;
    app.check_update_now();
    settle(&mut app, ready);
    assert!(app.sheet.refreshes > before, "the sheet was refreshed");
}
