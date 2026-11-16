use super::*;
use plith_audio::endpoints::{AttachedState, Endpoint};
use plith_audio::voicemeeter::{Rail, VmKind};
use plith_core::pages::pill::PillState;
use plith_media::{Artwork, Timeline};
use std::time::Duration;

fn chan(rail: Rail, index: usize, label: &str, routes: &[bool]) -> ChannelSnapshot {
    ChannelSnapshot {
        rail,
        index,
        label: label.to_string(),
        gain_db: -3.5,
        muted: index == 1,
        routes: routes.to_vec(),
    }
}

fn banana() -> VmSnapshot {
    let strips = (0..5)
        .map(|i| {
            chan(
                Rail::Strip,
                i,
                &format!("S{i}"),
                &[i == 0, false, false, i == 2, false],
            )
        })
        .collect();
    let buses = (0..5)
        .map(|i| chan(Rail::Bus, i, &format!("B{i}"), &[]))
        .collect();
    VmSnapshot {
        kind: VmKind::Banana,
        strips,
        buses,
        signal: vec![false, true, false, false, false],
    }
}

fn endpoints() -> EndpointState {
    let endpoint = |id: &str, friendly: &str, device: &str| Endpoint {
        id: id.to_string(),
        friendly_name: friendly.to_string(),
        device_name: device.to_string(),
    };
    EndpointState {
        default_id: Some("g733".to_string()),
        active: vec![
            endpoint("g733", "Speakers (Logitech G733)", "Logitech G733"),
            endpoint("hdmi", "DELL (NVIDIA High Definition Audio)", ""),
        ],
        attached: Some(AttachedState {
            id: "g733".to_string(),
            volume: 0.62,
            muted: false,
        }),
    }
}

fn mic(muted: bool) -> MicSnapshot {
    MicSnapshot {
        id: "mv7".to_string(),
        name: "SHURE MV7".to_string(),
        muted,
        volume: 0.8,
    }
}

#[test]
fn model_copies_vm_snapshot() {
    let facts = vm_facts(&banana());
    assert_eq!(facts.strips.len(), 5);
    assert_eq!(facts.buses.len(), 5);
    assert_eq!(facts.hardware_strips, 3);
    assert_eq!(facts.hardware_buses, 3);
    assert_eq!(
        facts.strips[2],
        ChannelFacts {
            label: "S2".to_string(),
            gain_db: -3.5,
            muted: false,
            routes: vec![false, false, false, true, false],
        }
    );
    assert!(facts.strips[1].muted);
    assert_eq!(facts.buses[4].label, "B4");
    assert!(facts.buses[4].routes.is_empty());
    assert_eq!(facts.signal, [false, true, false, false, false]);
}

#[test]
fn audio_facts_copy_endpoints_with_short_names() {
    let facts = audio_facts(Some(&banana()), &endpoints(), Some(&mic(true)));
    assert!(facts.vm.is_some());
    assert_eq!(facts.endpoints.len(), 2);
    assert_eq!(facts.endpoints[0].name, "Logitech G733");
    assert_eq!(
        facts.endpoints[1].name,
        "DELL (NVIDIA High Definition Audio)"
    );
    assert_eq!(facts.endpoints[1].id, "hdmi");
    assert_eq!(facts.default_id.as_deref(), Some("g733"));
    assert_eq!(facts.attached, Some((0.62, false)));
    assert_eq!(
        facts.mic,
        Some(MicFacts {
            name: "SHURE MV7".to_string(),
            muted: true,
        })
    );
    let bare = audio_facts(None, &EndpointState::default(), None);
    assert_eq!(bare, AudioFacts::default());
}

fn playing() -> MediaSnapshot {
    MediaSnapshot {
        has_session: true,
        source_id: "Spotify.exe".to_string(),
        title: "Midnight City".to_string(),
        artist: "M83".to_string(),
        thumbnail: None,
        artwork: Some(Arc::new(Artwork {
            width: 2,
            height: 1,
            bgra: Arc::from(vec![1_u8; 8]),
            id: 77,
        })),
        playing: true,
        can_seek: true,
        can_next: true,
        can_prev: false,
        timeline: Some(Timeline {
            position: Duration::from_secs(100),
            duration: Duration::from_secs(240),
            stamp: SystemTime::UNIX_EPOCH + Duration::from_secs(5000),
        }),
    }
}

#[test]
fn media_facts_copy_the_session_and_move_the_timeline() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(5002);
    let facts = media_facts(&playing(), Some("G733".to_string()), now);
    assert!(facts.has_session);
    assert_eq!(facts.source_name, "Spotify");
    assert_eq!(facts.title, "Midnight City");
    assert_eq!(facts.artist, "M83");
    assert_eq!(facts.art, Some(ArtId(77)));
    assert!(facts.playing && facts.can_seek && facts.can_next && !facts.can_prev);
    assert_eq!(facts.output.as_deref(), Some("G733"));
    let timeline = facts.timeline.unwrap();
    assert!((timeline.position_s - 102.0).abs() < 1e-9);
    assert!((timeline.duration_s - 240.0).abs() < 1e-9);
    assert_eq!(timeline.stamp, now);
    assert!(timeline.playing);
    let none = media_facts(&MediaSnapshot::none(), None, now);
    assert_eq!(none, MediaFacts::default());
}

#[test]
fn art_pixels_carry_the_artwork_id() {
    let art = art_pixels(&playing()).unwrap();
    assert_eq!(art.id, ArtId(77));
    assert_eq!((art.width, art.height), (2, 1));
    assert_eq!(art.bgra.len(), 8);
    assert_eq!(art_pixels(&MediaSnapshot::none()), None);
}

#[test]
fn system_facts_take_the_first_display_and_the_mic_bus() {
    let displays = [
        DeviceLevel {
            id: "a".to_string(),
            name: "DELL AW2725DF".to_string(),
            percent: 70,
        },
        DeviceLevel {
            id: "b".to_string(),
            name: "Other".to_string(),
            percent: 20,
        },
    ];
    let vm = vm_facts(&banana());
    let facts = system_facts(&displays, Some(&mic(false)), Some(&vm));
    assert_eq!(facts.brightness, Some(("DELL AW2725DF".to_string(), 70)));
    assert_eq!(
        facts.mic,
        Some(system::MicFacts {
            name: "HW 1 \u{b7} SHURE MV7".to_string(),
            muted: false,
        })
    );
    assert_eq!(facts.mic_bus.as_deref(), Some("A1"));
    let bare = system_facts(&[], Some(&mic(true)), None);
    assert_eq!(bare.brightness, None);
    assert_eq!(
        bare.mic.as_ref().map(|m| m.name.as_str()),
        Some("SHURE MV7")
    );
    assert_eq!(bare.mic_bus, None);
}

#[test]
fn facts_compose_the_pages_and_the_pill() {
    let readings = Readings {
        vm: None,
        endpoints: endpoints(),
        mic: None,
        media: playing(),
        brightness: Vec::new(),
    };
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(5000);
    let facts = facts(&readings, now);
    assert_eq!(facts.pill.output, "Logitech G733");
    assert_eq!(facts.pill.value, Some(62));
    assert_eq!(facts.pill.state, PillState::Live);
    assert_eq!(facts.media.output.as_deref(), Some("Logitech G733"));
    assert_eq!(facts.art.map(|a| a.id), Some(ArtId(77)));
    assert_eq!(facts.audio.endpoints.len(), 2);
    let quiet = Readings {
        media: MediaSnapshot::none(),
        endpoints: EndpointState::default(),
        ..readings
    };
    let facts = super::facts(&quiet, now);
    assert_eq!(facts.pill.state, PillState::Idle);
    assert_eq!(facts.media.output, None);
    // The mixer's held signal makes the pill live without media.
    let mixing = Readings {
        vm: Some(banana()),
        ..quiet
    };
    assert_eq!(super::facts(&mixing, now).pill.state, PillState::Live);
}
