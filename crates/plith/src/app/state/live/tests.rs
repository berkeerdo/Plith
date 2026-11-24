use super::*;
use crate::app::feed::{Feed, Timers};
use crate::app::saver::Saver;
use crate::app::state::Relayouter;
use crate::app::weather_link::WeatherLink;
use crate::log::{LogFile, Stamp};
use crate::services::fixture::FixtureServices;
use plith_core::display::Display;
use plith_core::lang::Lang;
use plith_core::launch::Fixture;
use plith_core::pages::idle_pill::rest_shape;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_core::settings::change::SettingChange;
use plith_core::settings::{IdleLook, LangChoice, LocationMode, parse};
use plith_core::theme::{Palette, Theme};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;

struct NoTimers;

impl Timers for NoTimers {
    fn set(&self, _id: usize, _ms: u32) {}
    fn kill(&self, _id: usize) {}
}

fn folder(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("plith-live-{name}-{}", std::process::id()))
}

fn stamp() -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 20,
        hour: 10,
        minute: 5,
        second: 0,
        millis: 0,
    }
}

/// An app on the banana fixture, in Turkish, with the idle model attached.
fn fed(name: &str) -> App {
    let log = LogFile::open_at(&folder(name).join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let mut app = App::new(
        mailbox,
        Arc::new(ShapeCell::default()),
        Arc::new(log),
        Arc::new(AtomicBool::new(false)),
    );
    let mut feed = Feed::new(
        Box::new(FixtureServices::new(Fixture::Banana)),
        Box::new(NoTimers),
        Box::new(stamp),
        Lang::Tr,
        &app.mailbox,
    );
    let idle = crate::app::idle_facts::Idle::new(Settings::default(), Lang::Tr);
    feed.set_idle(idle, &app.mailbox);
    app.attach_feed(feed);
    app
}

/// Drops `app` (it holds the log file open) and removes the test's folder.
fn clean(app: App, name: &str) {
    drop(app);
    std::fs::remove_dir_all(folder(name)).unwrap();
}

fn lang_of(app: &App) -> Lang {
    app.feed.as_ref().unwrap().lang()
}

#[test]
fn the_language_is_resolved_again_only_for_a_language_change() {
    let mut app = fed("lang");
    // A settings file naming German, applied with a look change only: the feed keeps Turkish.
    let german = Settings {
        lang: LangChoice::Code(String::from("de")),
        ..Settings::default()
    };
    app.apply_settings(
        &german,
        Effects {
            pill: true,
            ..Effects::default()
        },
    );
    assert_eq!(lang_of(&app), Lang::Tr);
    // The language change itself resolves the new choice before the feed rebuilds.
    app.change(SettingChange::Lang(LangChoice::Code(String::from("de"))));
    assert_eq!(lang_of(&app), Lang::De);
    app.change(SettingChange::Lang(LangChoice::Code(String::from("fr"))));
    assert_eq!(lang_of(&app), Lang::Fr);
    clean(app, "lang");
}

/// An app whose feed reads `link`, with weather on and a network wiring.
fn with_link(name: &str) -> App {
    let mut app = fed(name);
    let wiring = WeatherWiring::new(Arc::clone(&app.log), Arc::new(|| {}), true);
    app.attach_weather(wiring, false);
    app.settings.weather = true;
    app.settings.location = LocationMode::City;
    let (_tx, rx) = std::sync::mpsc::channel();
    app.feed
        .as_mut()
        .unwrap()
        .set_weather(Some(WeatherLink::from_feed(rx)));
    app
}

#[test]
fn a_place_change_keeps_the_link_and_weather_off_drops_it() {
    let mut app = with_link("swap");
    app.change(SettingChange::City(String::from("Bursa")));
    let link = app.feed.as_mut().unwrap().take_weather();
    assert!(
        link.is_some(),
        "a running link is reconfigured, not replaced"
    );
    app.feed.as_mut().unwrap().set_weather(link);
    app.change(SettingChange::Weather(false));
    assert!(app.feed.as_mut().unwrap().take_weather().is_none());
    clean(app, "swap");
}

#[test]
fn a_change_applies_at_once_and_is_saved() {
    let mut app = fed("save");
    let path = folder("save").join("settings.ini");
    app.attach_saver(Saver::start(
        path.clone(),
        Box::new(|| {}),
        Box::new(|_| {}),
    ));
    app.change(SettingChange::Look(IdleLook::Clock));
    assert_eq!(app.settings.look, IdleLook::Clock);
    assert_eq!(app.mailbox.latest().scene.rest, rest_shape(IdleLook::Clock));
    app.flush_saver();
    let saved = parse(&std::fs::read_to_string(&path).unwrap()).0;
    assert_eq!(saved.look, IdleLook::Clock);
    clean(app, "save");
}

#[test]
fn a_change_without_a_saver_still_applies() {
    let mut app = fed("nosave");
    app.change(SettingChange::Look(IdleLook::Plain));
    assert_eq!(app.settings.look, IdleLook::Plain);
    app.flush_saver();
    clean(app, "nosave");
}

/// Saves go to the returned path; the app shows `Clock` while the file holds `Karot`.
fn pinned(name: &str) -> (App, PathBuf) {
    let mut app = fed(name);
    let path = folder(name).join("settings.ini");
    app.attach_saver(Saver::start(
        path.clone(),
        Box::new(|| {}),
        Box::new(|_| {}),
    ));
    app.settings.look = IdleLook::Clock;
    app.keep_file_look(Some(IdleLook::Karot));
    (app, path)
}

fn saved_look(path: &std::path::Path) -> IdleLook {
    parse(&std::fs::read_to_string(path).unwrap()).0.look
}

#[test]
fn the_command_line_look_is_not_saved_with_another_change() {
    let (mut app, path) = pinned("cli-look");
    app.change(SettingChange::Plays(false));
    app.flush_saver();
    assert_eq!(saved_look(&path), IdleLook::Karot, "the file's own look");
    assert_eq!(
        app.settings.look,
        IdleLook::Clock,
        "this run still shows it"
    );
    clean(app, "cli-look");
}

#[test]
fn picking_a_look_saves_it_even_when_it_is_the_one_shown() {
    let (mut app, path) = pinned("cli-pick");
    app.change(SettingChange::Look(IdleLook::Clock));
    app.change(SettingChange::Plays(false));
    app.flush_saver();
    assert_eq!(saved_look(&path), IdleLook::Clock);
    clean(app, "cli-pick");
}

#[test]
fn every_change_refreshes_an_open_settings_window() {
    let mut app = fed("refresh");
    let before = app.sheet.refreshes;
    app.change(SettingChange::Look(IdleLook::Plain));
    assert_eq!(app.sheet.refreshes, before + 1);
    app.change(SettingChange::Look(IdleLook::Plain));
    assert_eq!(app.sheet.refreshes, before + 1, "nothing changed");
    clean(app, "refresh");
}

/// Weather on with a running link and no network, so applying a change stops the link.
fn without_network(name: &str) -> App {
    let mut app = with_link(name);
    let wiring = WeatherWiring::new(Arc::clone(&app.log), Arc::new(|| {}), false);
    app.attach_weather(wiring, false);
    app
}

#[test]
fn a_city_committed_at_exit_is_saved_but_not_applied() {
    let mut app = without_network("exit-city");
    let path = folder("exit-city").join("settings.ini");
    app.attach_saver(Saver::start(
        path.clone(),
        Box::new(|| {}),
        Box::new(|_| {}),
    ));
    app.save_change(SettingChange::City(String::from("Bursa")));
    assert!(
        app.feed.as_mut().unwrap().take_weather().is_some(),
        "the weather was not reconfigured"
    );
    app.flush_saver();
    let saved = parse(&std::fs::read_to_string(&path).unwrap()).0;
    assert_eq!(saved.city, "Bursa");
    clean(app, "exit-city");
}

#[test]
fn a_city_change_while_running_does_apply() {
    let mut app = without_network("run-city");
    app.change(SettingChange::City(String::from("Bursa")));
    assert!(app.feed.as_mut().unwrap().take_weather().is_none());
    clean(app, "run-city");
}

/// Records the palettes the render thread would be sent.
struct FakePainter(Rc<RefCell<Vec<Palette>>>);

impl Relayouter for FakePainter {
    fn relayout(&self, _display: Display) {}

    fn set_palette(&self, palette: Palette) {
        self.0.borrow_mut().push(palette);
    }
}

fn with_painter(name: &str) -> (App, Rc<RefCell<Vec<Palette>>>) {
    let mut app = fed(name);
    let sent = Rc::new(RefCell::new(Vec::new()));
    app.attach_relayout(FakePainter(Rc::clone(&sent)));
    (app, sent)
}

#[test]
fn a_theme_change_sends_the_palette_once_and_the_same_theme_sends_nothing() {
    let (mut app, sent) = with_painter("theme-send");
    app.change(SettingChange::Theme(Theme::Kagit));
    assert_eq!(app.settings.theme, Theme::Kagit);
    assert_eq!(*sent.borrow(), vec![Theme::Kagit.palette()]);
    app.change(SettingChange::Theme(Theme::Kagit));
    app.change(SettingChange::Look(IdleLook::Plain));
    assert_eq!(sent.borrow().len(), 1, "only a theme change repaints");
    clean(app, "theme-send");
}

#[test]
fn a_theme_change_is_saved_and_refreshes_the_window() {
    let mut app = fed("theme-save");
    let path = folder("theme-save").join("settings.ini");
    app.attach_saver(Saver::start(
        path.clone(),
        Box::new(|| {}),
        Box::new(|_| {}),
    ));
    let before = app.sheet.refreshes;
    app.change(SettingChange::Theme(Theme::York));
    assert_eq!(app.sheet.refreshes, before + 1);
    app.flush_saver();
    let saved = parse(&std::fs::read_to_string(&path).unwrap()).0;
    assert_eq!(saved.theme, Theme::York);
    clean(app, "theme-save");
}

/// Saves go to the returned path; the app shows `Sodyum` while the file holds `Gece`.
fn theme_pinned(name: &str) -> (App, PathBuf) {
    let mut app = fed(name);
    let path = folder(name).join("settings.ini");
    app.attach_saver(Saver::start(
        path.clone(),
        Box::new(|| {}),
        Box::new(|_| {}),
    ));
    app.settings.theme = Theme::Sodyum;
    app.keep_file_theme(Some(Theme::Gece));
    (app, path)
}

fn saved_theme(path: &std::path::Path) -> Theme {
    parse(&std::fs::read_to_string(path).unwrap()).0.theme
}

#[test]
fn the_command_line_theme_is_not_saved_with_another_change() {
    let (mut app, path) = theme_pinned("cli-theme");
    app.change(SettingChange::Plays(false));
    app.flush_saver();
    assert_eq!(saved_theme(&path), Theme::Gece, "the file's own theme");
    assert_eq!(app.settings.theme, Theme::Sodyum, "this run still shows it");
    clean(app, "cli-theme");
}

#[test]
fn picking_a_theme_saves_it_even_when_it_is_the_one_shown() {
    let (mut app, path) = theme_pinned("cli-theme-pick");
    app.change(SettingChange::Theme(Theme::Sodyum));
    app.change(SettingChange::Plays(false));
    app.flush_saver();
    assert_eq!(saved_theme(&path), Theme::Sodyum);
    clean(app, "cli-theme-pick");
}
