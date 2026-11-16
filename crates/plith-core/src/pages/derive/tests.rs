use super::*;
use crate::pages::audio::{ChannelFacts, EndpointFacts};
use crate::pages::fixtures;
use std::time::Duration;

#[test]
fn short_name_prefers_the_device_description() {
    assert_eq!(
        short_name("Logitech G733", "Speakers (Logitech G733)"),
        "Logitech G733"
    );
    assert_eq!(short_name("", "Speakers (Realtek)"), "Speakers (Realtek)");
    assert_eq!(short_name("  ", "Speakers"), "Speakers");
}

#[test]
fn bus_tags_count_hardware_then_virtual() {
    let vm = fixtures::banana().vm.unwrap();
    assert_eq!(bus_tag(&vm, 0), "A1");
    assert_eq!(bus_tag(&vm, 1), "A2");
    assert_eq!(bus_tag(&vm, 2), "B1");
    assert_eq!(bus_tag(&vm, 3), "B2");
}

#[test]
fn mic_bus_is_the_first_bus_of_the_first_strip() {
    let mut vm = fixtures::banana().vm.unwrap();
    assert_eq!(mic_bus(&vm).as_deref(), Some("B1"));
    vm.strips[0].routes = vec![false; 4];
    assert_eq!(mic_bus(&vm), None);
    vm.strips.clear();
    assert_eq!(mic_bus(&vm), None);
}

#[test]
fn mic_caption_names_the_slot_only_with_voicemeeter() {
    assert_eq!(mic_caption("SHURE MV7", true), "HW 1 \u{b7} SHURE MV7");
    assert_eq!(mic_caption("SHURE MV7", false), "SHURE MV7");
}

#[test]
fn signal_needs_a_peak_above_the_threshold() {
    assert!(!any_signal(&[]));
    assert!(!any_signal(&[[0.0, 0.01]]));
    assert!(any_signal(&[[0.0, 0.0], [0.0, 0.2]]));
}

#[test]
fn pill_state_muted_beats_live_beats_idle() {
    assert_eq!(pill_state(true, true, true), PillState::Muted);
    assert_eq!(pill_state(false, true, false), PillState::Live);
    assert_eq!(pill_state(false, false, true), PillState::Live);
    assert_eq!(pill_state(false, false, false), PillState::Idle);
}

#[test]
fn pill_facts_read_the_default_endpoint() {
    let audio = fixtures::no_voicemeeter();
    let pill = pill_facts(&audio, true);
    assert_eq!(pill.output, "Logitech G733");
    assert_eq!(pill.value, Some(62));
    assert_eq!(pill.state, PillState::Live);
    let muted = AudioFacts {
        attached: Some((0.5, true)),
        ..audio
    };
    assert_eq!(pill_facts(&muted, true).state, PillState::Muted);
    let none = pill_facts(&AudioFacts::default(), false);
    assert_eq!(none.output, "");
    assert_eq!(none.value, None);
    assert_eq!(none.state, PillState::Idle);
}

#[test]
fn pill_facts_without_a_known_default_show_no_name() {
    let audio = AudioFacts {
        endpoints: vec![EndpointFacts {
            id: "a".to_string(),
            name: "A".to_string(),
        }],
        default_id: Some("gone".to_string()),
        vm: Some(VmFacts {
            strips: vec![ChannelFacts::default()],
            ..VmFacts::default()
        }),
        ..AudioFacts::default()
    };
    assert_eq!(pill_facts(&audio, false).output, "");
}

#[test]
fn source_label_reads_app_ids() {
    assert_eq!(
        source_label("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
        "Spotify"
    );
    assert_eq!(source_label("Spotify.exe"), "Spotify");
    assert_eq!(
        source_label("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
        "ZuneMusic"
    );
    assert_eq!(source_label(r"C:\Apps\foobar2000.EXE"), "foobar2000");
    assert_eq!(source_label("chrome"), "chrome");
    assert_eq!(source_label(""), "");
}

#[test]
fn rebase_moves_a_playing_timeline_to_now() {
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let playing = SeekTimeline {
        position_s: 10.0,
        duration_s: 100.0,
        stamp,
        playing: true,
    };
    let now = stamp + Duration::from_millis(2500);
    let moved = rebase(playing, now);
    assert!((moved.position_s - 12.5).abs() < 1e-9);
    assert_eq!(moved.stamp, now);
    assert!((moved.fraction_at(now) - playing.fraction_at(now)).abs() < 1e-9);
    let late = rebase(playing, stamp + Duration::from_secs(500));
    assert!((late.position_s - 100.0).abs() < 1e-9);
    let paused = SeekTimeline {
        playing: false,
        ..playing
    };
    assert_eq!(rebase(paused, now), paused);
    let early = rebase(playing, stamp - Duration::from_secs(1));
    assert!((early.position_s - 10.0).abs() < 1e-9);
}

#[test]
fn pill_is_live_while_a_strip_holds_signal() {
    let mut audio = fixtures::banana();
    assert_eq!(pill_facts(&audio, false).state, PillState::Idle);
    audio.vm.as_mut().unwrap().signal = vec![false, true, false, false];
    assert_eq!(pill_facts(&audio, false).state, PillState::Live);
    audio.vm.as_mut().unwrap().signal = vec![false; 4];
    assert_eq!(pill_facts(&audio, false).state, PillState::Idle);
    assert_eq!(pill_facts(&audio, true).state, PillState::Live);
}

#[test]
fn rebase_to_second_is_equal_within_a_second() {
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let playing = SeekTimeline {
        position_s: 10.0,
        duration_s: 100.0,
        stamp,
        playing: true,
    };
    let early = rebase_to_second(playing, stamp + Duration::from_millis(2200));
    let late = rebase_to_second(playing, stamp + Duration::from_millis(2900));
    assert_eq!(early, late);
    assert!((early.position_s - 12.0).abs() < 1e-9);
    assert_eq!(early.stamp, stamp + Duration::from_secs(2));
    // The same moment on the track, so the painter's extrapolation does not jump.
    let now = stamp + Duration::from_millis(2900);
    assert!((late.fraction_at(now) - playing.fraction_at(now)).abs() < 1e-9);
    let next = rebase_to_second(playing, stamp + Duration::from_millis(3100));
    assert!((next.position_s - 13.0).abs() < 1e-9);
}

#[test]
fn rebase_to_second_keeps_a_timeline_that_has_not_reached_a_second() {
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let playing = SeekTimeline {
        position_s: 10.4,
        duration_s: 100.0,
        stamp,
        playing: true,
    };
    assert_eq!(
        rebase_to_second(playing, stamp + Duration::from_millis(300)),
        playing
    );
    let paused = SeekTimeline {
        playing: false,
        ..playing
    };
    assert_eq!(
        rebase_to_second(paused, stamp + Duration::from_secs(9)),
        paused
    );
    // Past the end the result no longer moves.
    let near_end = SeekTimeline {
        position_s: 99.5,
        ..playing
    };
    let a = rebase_to_second(near_end, stamp + Duration::from_secs(5));
    let b = rebase_to_second(near_end, stamp + Duration::from_secs(7));
    assert_eq!(a, b);
    assert!((a.position_s - 100.0).abs() < 1e-9);
}

#[test]
fn rebase_to_second_floors_a_stream_with_no_length() {
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let stream = SeekTimeline {
        position_s: 10.0,
        duration_s: 0.0,
        stamp,
        playing: true,
    };
    let early = rebase_to_second(stream, stamp + Duration::from_millis(2200));
    let late = rebase_to_second(stream, stamp + Duration::from_millis(2900));
    assert_eq!(early, late, "a zero duration is no end: whole seconds only");
    assert!((early.position_s - 12.0).abs() < 1e-9);
}
