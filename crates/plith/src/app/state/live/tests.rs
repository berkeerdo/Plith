use super::*;
use crate::app::feed::{Feed, Timers};
use crate::app::saver::Saver;
use crate::app::weather_link::WeatherLink;
use crate::log::{LogFile, Stamp};
use crate::services::fixture::FixtureServices;
use plith_core::lang::Lang;
use plith_core::launch::Fixture;
use plith_core::pages::idle_pill::rest_shape;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_core::settings::change::SettingChange;
use plith_core::settings::{IdleLook, LangChoice, LocationMode, parse};
use std::path::PathBuf;
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
