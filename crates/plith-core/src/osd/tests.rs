use super::*;
use crate::lang::{Lang, Text, text};
use crate::pages::audio::EndpointFacts;

fn audio(attached: Option<(f32, bool)>) -> AudioFacts {
    AudioFacts {
        endpoints: vec![
            EndpointFacts {
                id: "a".to_string(),
                name: "Speakers".to_string(),
                ..EndpointFacts::default()
            },
            EndpointFacts {
                id: "b".to_string(),
                name: "Headset".to_string(),
                ..EndpointFacts::default()
            },
        ],
        default_id: Some("a".to_string()),
        attached,
        ..AudioFacts::default()
    }
}

fn system(brightness: Option<u8>) -> SystemFacts {
    SystemFacts {
        brightness: brightness.map(|percent| ("Dell U2723".to_string(), percent)),
        ..SystemFacts::default()
    }
}

fn read(volume: Option<(f32, bool)>, brightness: Option<u8>) -> OsdReading {
    reading(&audio(volume), &system(brightness), Lang::En)
}

#[test]
fn percent_and_mute_changes_fire() {
    let prev = read(Some((0.5, false)), Some(40));
    let next = read(Some((0.6, false)), Some(50));
    let fired = changes(&prev, &next, None);
    assert_eq!(fired.len(), 2);
    assert_eq!(fired[0].meter, OsdMeter::Volume);
    assert_eq!(fired[0].percent, 60);
    assert_eq!(fired[0].label, "Speakers");
    assert_eq!(fired[1].meter, OsdMeter::Brightness);
    assert_eq!(fired[1].percent, 50);
    assert_eq!(fired[1].label, "Dell U2723");
    assert!(!fired[1].muted);

    let muted = read(Some((0.5, true)), Some(40));
    let fired = changes(&prev, &muted, None);
    assert_eq!(fired.len(), 1);
    assert!(fired[0].muted);
}

#[test]
fn same_values_fire_nothing() {
    let prev = read(Some((0.5, false)), Some(40));
    assert!(changes(&prev, &prev.clone(), None).is_empty());
}

#[test]
fn appearing_meter_is_not_a_change() {
    let prev = read(None, None);
    let next = read(Some((0.5, false)), Some(40));
    assert!(changes(&prev, &next, None).is_empty());
}

#[test]
fn device_switch_with_same_level_fires_nothing() {
    let prev = read(Some((0.5, false)), None);
    let mut switched = audio(Some((0.5, false)));
    switched.default_id = Some("b".to_string());
    let next = reading(&switched, &system(None), Lang::En);
    assert_eq!(next.volume.as_ref().unwrap().label, "Headset");
    assert!(changes(&prev, &next, None).is_empty());
}

#[test]
fn open_audio_page_hides_volume_only() {
    let prev = read(Some((0.5, false)), Some(40));
    let next = read(Some((0.6, false)), Some(50));
    let fired = changes(&prev, &next, Some(PageId::Audio));
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].meter, OsdMeter::Brightness);
}

#[test]
fn open_system_page_hides_brightness_only() {
    let prev = read(Some((0.5, false)), Some(40));
    let next = read(Some((0.6, false)), Some(50));
    let fired = changes(&prev, &next, Some(PageId::System));
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].meter, OsdMeter::Volume);
    let other = changes(&prev, &next, Some(PageId::Media));
    assert_eq!(other.len(), 2);
}

#[test]
fn long_label_is_cut_on_a_char_boundary() {
    let mut facts = audio(Some((0.5, false)));
    facts.endpoints[0].name = "ğ".repeat(35);
    assert_eq!(facts.endpoints[0].name.len(), 70);
    let level = reading(&facts, &system(None), Lang::En).volume.unwrap();
    assert!(level.label.len() <= MAX_LABEL_BYTES);
    assert!(!level.label.is_empty());
    assert!(level.label.chars().all(|c| c == 'ğ'));
}

#[test]
fn missing_default_name_uses_tab_audio() {
    let mut facts = audio(Some((0.5, false)));
    facts.default_id = None;
    let level = reading(&facts, &system(None), Lang::Tr).volume.unwrap();
    assert_eq!(level.label, "SES");
    facts.default_id = Some("unknown".to_string());
    let level = reading(&facts, &system(None), Lang::Tr).volume.unwrap();
    assert_eq!(level.label, "SES");
}

#[test]
fn fallback_label_follows_the_language() {
    let mut facts = audio(Some((0.5, false)));
    facts.default_id = None;
    for lang in Lang::ALL {
        let level = reading(&facts, &system(None), lang).volume.unwrap();
        assert_eq!(level.label, text(Text::TabAudio, lang), "{lang:?}");
    }
    let english = reading(&facts, &system(None), Lang::En).volume.unwrap();
    assert_eq!(english.label, "AUDIO");
}

#[test]
fn the_muted_word_follows_the_language_and_fits_the_wire() {
    let facts = audio(Some((0.5, true)));
    for lang in Lang::ALL {
        let level = reading(&facts, &system(None), lang).volume.unwrap();
        assert_eq!(level.muted_word, text(Text::Muted, lang), "{lang:?}");
        assert!(level.muted_word.len() <= MAX_LABEL_BYTES);
    }
}

#[test]
fn percent_rounds_and_clamps() {
    let percent = |value: f32| {
        reading(&audio(Some((value, false))), &system(None), Lang::En)
            .volume
            .unwrap()
            .percent
    };
    assert_eq!(percent(0.004), 0);
    assert_eq!(percent(0.995), 100);
    assert_eq!(percent(1.2), 100);
    assert_eq!(percent(-0.3), 0);
}
