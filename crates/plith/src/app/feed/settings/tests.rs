use super::super::tests::{open_page, page_labels, rig, stamp};
use crate::app::idle_facts::Idle;
use crate::services::fixture::idle as mock;
use plith_core::lang::Lang;
use plith_core::pages::PageId;
use plith_core::settings::change::Effects;
use plith_core::settings::{LangChoice, Settings, TempUnit};
use plith_core::weather::Stamp as WeatherStamp;
use std::sync::Arc;

fn weather_on() -> Settings {
    Settings {
        weather: true,
        ..Settings::default()
    }
}

/// An idle model with the mock forecast of the rig's day, so temperatures show.
fn idle_with_forecast(settings: Settings, lang: Lang) -> Idle {
    let now = stamp();
    let day = (now.year, now.month, now.day);
    let read_at = WeatherStamp {
        day,
        hour: 9,
        minute: 0,
    };
    Idle::new(settings, lang).with_forecast(Some(mock::forecast(day, read_at)))
}

#[test]
fn lang_switch_keeps_page() {
    let mut rig = rig();
    rig.feed
        .set_idle(Idle::new(Settings::default(), Lang::Tr), &rig.mailbox);
    open_page(&rig, PageId::Audio.index());
    rig.feed.on_wake(&rig.mailbox);
    let before = page_labels(&rig.mailbox, PageId::Audio.index());
    let list = Arc::clone(&rig.mailbox.latest().content.pages[PageId::Audio.index()]);

    let settings = Settings {
        lang: LangChoice::Code("en".into()),
        ..Settings::default()
    };
    let effects = Effects {
        lang: true,
        ..Effects::default()
    };
    rig.feed
        .apply_settings(&settings, Lang::En, effects, &rig.mailbox);

    let scene = rig.mailbox.latest().scene;
    assert!(scene.expanded, "a language switch never collapses");
    assert_eq!(scene.page, PageId::Audio.index());
    assert_eq!(rig.feed.lang(), Lang::En);
    let after_list = &rig.mailbox.latest().content.pages[PageId::Audio.index()];
    assert!(
        !Arc::ptr_eq(&list, after_list),
        "the audio page was rebuilt"
    );
    let after = page_labels(&rig.mailbox, PageId::Audio.index());
    assert_ne!(before, after);
    assert!(before.iter().any(|l| l == "MEDYA"), "{before:?}");
    assert!(after.iter().any(|l| l == "MEDIA"), "{after:?}");
}

#[test]
fn unit_change_rebuilds_welcome_only() {
    let mut rig = rig();
    rig.feed
        .set_idle(idle_with_forecast(weather_on(), Lang::En), &rig.mailbox);
    open_page(&rig, PageId::Welcome.index());
    rig.feed.on_wake(&rig.mailbox);
    let before = rig.mailbox.latest().content;

    let settings = Settings {
        unit: TempUnit::F,
        ..weather_on()
    };
    let effects = Effects {
        welcome: true,
        pill: true,
        ..Effects::default()
    };
    rig.feed
        .apply_settings(&settings, Lang::En, effects, &rig.mailbox);

    let after = rig.mailbox.latest().content;
    let welcome = PageId::Welcome.index();
    assert!(
        !Arc::ptr_eq(&before.pages[welcome], &after.pages[welcome]),
        "the welcome view shows the new unit"
    );
    for page in [PageId::Audio, PageId::Media, PageId::Shelf, PageId::System] {
        let i = page.index();
        assert!(
            Arc::ptr_eq(&before.pages[i], &after.pages[i]),
            "{page:?} kept its list"
        );
    }
    assert_eq!(rig.feed.lang(), Lang::En);
}

#[test]
fn a_closed_island_rebuilds_the_pill_and_keeps_pages_for_the_opening() {
    let mut rig = rig();
    rig.feed
        .set_idle(Idle::new(Settings::default(), Lang::Tr), &rig.mailbox);
    rig.feed.on_wake(&rig.mailbox);
    let audio = PageId::Audio.index();
    let list = Arc::clone(&rig.mailbox.latest().content.pages[audio]);
    let effects = Effects {
        lang: true,
        ..Effects::default()
    };
    rig.feed
        .apply_settings(&Settings::default(), Lang::En, effects, &rig.mailbox);
    // Closed: the pages wait (G2).
    assert!(Arc::ptr_eq(
        &list,
        &rig.mailbox.latest().content.pages[audio]
    ));
    open_page(&rig, audio);
    rig.feed.on_wake(&rig.mailbox);
    let labels = page_labels(&rig.mailbox, audio);
    assert!(labels.iter().any(|l| l == "MEDIA"), "{labels:?}");
}

#[test]
fn a_look_change_moves_the_rest_shape() {
    use plith_core::pages::idle_pill::rest_shape;
    use plith_core::settings::IdleLook;
    let mut rig = rig();
    rig.feed
        .set_idle(Idle::new(Settings::default(), Lang::En), &rig.mailbox);
    let settings = Settings {
        look: IdleLook::Hidden,
        ..Settings::default()
    };
    let effects = Effects {
        pill: true,
        ..Effects::default()
    };
    rig.feed
        .apply_settings(&settings, Lang::En, effects, &rig.mailbox);
    assert_eq!(
        rig.mailbox.latest().scene.rest,
        rest_shape(IdleLook::Hidden)
    );
}
