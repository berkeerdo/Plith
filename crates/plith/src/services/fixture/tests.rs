use super::*;
use plith_core::pages::pill::PillState;

#[test]
fn a_new_fixture_has_nothing_to_drain() {
    let mut fixture = FixtureServices::new(Fixture::Banana);
    assert_eq!(fixture.drain(), Changes::NONE);
}

#[test]
fn banana_facts_are_the_mock() {
    let fixture = FixtureServices::new(Fixture::Banana);
    let facts = fixture.facts(1.0);
    let vm = facts.audio.vm.as_ref().unwrap();
    assert_eq!(vm.strips.len(), 4);
    assert_eq!(facts.audio.default_id.as_deref(), Some("g733"));
    assert_eq!(
        facts.system.brightness,
        Some(("DELL AW2725DF".to_string(), 70))
    );
    assert_eq!(facts.system.mic_bus.as_deref(), Some("B1"));
    assert_eq!(
        facts.system.mic.as_ref().map(|m| m.name.as_str()),
        Some("HW 1 \u{b7} SHURE MV7")
    );
    assert!(facts.media.playing);
    assert_eq!(facts.media.art, Some(plith_core::pages::fixtures::ART_ID));
    assert_eq!(
        facts.art.map(|a| a.id),
        Some(plith_core::pages::fixtures::ART_ID)
    );
    assert_eq!(facts.pill.state, PillState::Live);
    assert_eq!(facts.pill.value, Some(62));
}

#[test]
fn potato_and_bare_differ_in_the_mixer() {
    let potato = FixtureServices::new(Fixture::Potato).facts(0.0);
    assert_eq!(potato.audio.vm.as_ref().unwrap().strips.len(), 8);
    let bare = FixtureServices::new(Fixture::Bare);
    let facts = bare.facts(0.0);
    assert!(facts.audio.vm.is_none());
    assert_eq!(facts.audio.endpoints.len(), 2);
    assert_eq!(facts.system.mic_bus, None);
    assert_eq!(
        facts.system.mic.as_ref().map(|m| m.name.as_str()),
        Some("SHURE MV7")
    );
    assert_eq!(bare.levels(), None);
    assert_eq!((bare.level_source())(), None);
}

#[test]
fn fixture_writes_stay_in_fixture() {
    let mut fixture = FixtureServices::new(Fixture::Banana);
    let before = fixture.facts(0.0);
    assert!(!before.audio.vm.as_ref().unwrap().strips[0].routes[0]);

    fixture.toggle_route(0, 0);
    assert_eq!(fixture.drain(), Changes::AUDIO);
    assert_eq!(fixture.drain(), Changes::NONE, "a change drains once");
    let facts = fixture.facts(0.0);
    assert!(facts.audio.vm.as_ref().unwrap().strips[0].routes[0]);
    assert_eq!(facts.system.mic_bus.as_deref(), Some("A1"));

    fixture.toggle_mic();
    assert!(fixture.drain().has(Changes::AUDIO));
    let facts = fixture.facts(0.0);
    assert!(facts.audio.mic.as_ref().unwrap().muted);
    assert!(facts.system.mic.as_ref().unwrap().muted);

    fixture.set_brightness(30);
    assert_eq!(fixture.drain(), Changes::SYSTEM);
    assert_eq!(
        fixture.facts(0.0).system.brightness,
        Some(("DELL AW2725DF".to_string(), 30))
    );

    fixture.set_default_endpoint("r1280");
    assert_eq!(fixture.drain(), Changes::AUDIO);
    assert_eq!(
        fixture.facts(0.0).audio.default_id.as_deref(),
        Some("r1280")
    );
    fixture.set_default_endpoint("missing");
    assert_eq!(fixture.drain(), Changes::NONE);
}

#[test]
fn fixture_media_writes_stay_in_fixture() {
    let mut fixture = FixtureServices::new(Fixture::Banana);
    fixture.play_pause();
    assert_eq!(fixture.drain(), Changes::MEDIA);
    assert!(!fixture.facts(0.0).media.playing);

    fixture.seek(Duration::from_secs(10));
    assert_eq!(fixture.drain(), Changes::MEDIA);
    let timeline = fixture.facts(0.0).media.timeline.unwrap();
    assert!((timeline.position_s - 10.0).abs() < 1e-9);

    fixture.next();
    assert_eq!(fixture.drain(), Changes::MEDIA);
    let timeline = fixture.facts(0.0).media.timeline.unwrap();
    assert!(timeline.position_s.abs() < 1e-9);
    fixture.previous();
    assert_eq!(fixture.drain(), Changes::MEDIA);
}

#[test]
fn writes_wake_the_ui_thread() {
    let wakes = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = std::rc::Rc::clone(&wakes);
    let mut fixture = FixtureServices::new(Fixture::Banana)
        .with_waker(Box::new(move || seen.set(seen.get() + 1)));
    fixture.toggle_mic();
    fixture.set_brightness(5);
    fixture.set_default_endpoint("missing");
    assert_eq!(wakes.get(), 2, "only a real change wakes");
}

#[test]
fn out_of_range_writes_change_nothing() {
    let mut fixture = FixtureServices::new(Fixture::Banana);
    fixture.toggle_route(9, 0);
    fixture.toggle_route(0, 9);
    assert_eq!(fixture.drain(), Changes::NONE);
    let mut bare = FixtureServices::new(Fixture::Bare);
    bare.toggle_route(0, 0);
    assert_eq!(bare.drain(), Changes::NONE);
}

#[test]
fn fixture_levels_are_deterministic_and_silent_when_muted() {
    let muted = [false, true, false];
    let a = fixture_levels(&muted, 4, 2.5);
    assert_eq!(a, fixture_levels(&muted, 4, 2.5));
    assert_eq!(a.strips.len(), 3);
    assert_eq!(a.buses.len(), 4);
    assert!(a.strips[1].iter().all(|v| v.abs() < f32::EPSILON));
    for [l, r] in a.strips.iter().chain(&a.buses) {
        assert!((0.0..=1.0).contains(l) && (0.0..=1.0).contains(r));
    }
    assert!(a.strips[0][0] > 0.01, "an open strip carries signal");
    assert_ne!(a, fixture_levels(&muted, 4, 3.0), "the levels move");
}

fn with_content() -> (FixtureServices, std::sync::mpsc::Sender<u8>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (FixtureServices::new(Fixture::Banana).with_content(rx), tx)
}

#[test]
fn content_one_is_the_next_track_with_new_art() {
    let (mut fixture, tx) = with_content();
    let before = fixture.facts(0.0);
    tx.send(1).unwrap();
    assert_eq!(fixture.drain(), Changes::MEDIA);
    let after = fixture.facts(0.0);
    assert_ne!(after.media.title, before.media.title);
    assert_ne!(after.media.art, before.media.art);
    assert_eq!(
        after.art.map(|a| a.id),
        Some(plith_core::pages::fixtures::ART_NEXT_ID)
    );
    assert_eq!(after.audio, before.audio);
    assert_eq!(after.system, before.system);
}

#[test]
fn content_two_toggles_a_route_and_mutes_a_source() {
    let (mut fixture, tx) = with_content();
    let before = fixture.facts(0.0);
    tx.send(2).unwrap();
    assert_eq!(fixture.drain(), Changes::AUDIO);
    let after = fixture.facts(0.0);
    let (b, a) = (before.audio.vm.unwrap(), after.audio.vm.unwrap());
    assert_ne!(a.strips, b.strips);
    let routes = |vm: &plith_core::pages::audio::VmFacts| {
        vm.strips
            .iter()
            .map(|s| s.routes.clone())
            .collect::<Vec<_>>()
    };
    assert_ne!(routes(&a), routes(&b), "a route changed");
    let muted =
        |vm: &plith_core::pages::audio::VmFacts| vm.strips.iter().filter(|s| s.muted).count();
    assert_ne!(muted(&a), muted(&b), "a source is muted");
    assert_eq!(after.media.title, before.media.title);
    assert_eq!(after.media.art, before.media.art);
}

#[test]
fn content_three_changes_the_brightness_only() {
    let (mut fixture, tx) = with_content();
    let before = fixture.facts(0.0);
    tx.send(3).unwrap();
    assert_eq!(fixture.drain(), Changes::SYSTEM);
    let after = fixture.facts(0.0);
    assert_ne!(after.system.brightness, before.system.brightness);
    assert_eq!(after.audio, before.audio);
}

#[test]
fn content_zero_restores_the_fixture_as_launched() {
    let (mut fixture, tx) = with_content();
    let launched = fixture.facts(0.0);
    for n in [1, 2, 3] {
        tx.send(n).unwrap();
        assert!(fixture.drain().any());
    }
    tx.send(0).unwrap();
    assert!(fixture.drain().any());
    let back = fixture.facts(0.0);
    assert_eq!(back.audio, launched.audio);
    assert_eq!(back.system, launched.system);
    assert_eq!(back.media.title, launched.media.title);
    assert_eq!(back.media.art, launched.media.art);
}

#[test]
fn repeating_a_variant_or_an_unknown_one_changes_nothing() {
    let (mut fixture, tx) = with_content();
    tx.send(1).unwrap();
    let _ = fixture.drain();
    tx.send(1).unwrap();
    tx.send(9).unwrap();
    assert_eq!(fixture.drain(), Changes::NONE);
}
