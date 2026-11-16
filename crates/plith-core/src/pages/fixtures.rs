//! The approved mock's data as facts (`docs/design/r3/ray-paneli.html`), for offscreen renders
//! and the demo. Pure: no Windows code, nothing read from the machine.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::ambient::{LiveValues, TraceSamples};
use crate::draw::{ArtId, ArtPixels, SeekTimeline};
use crate::pages::audio::{Activity, AudioFacts, ChannelFacts, EndpointFacts, VmFacts};
use crate::pages::media::MediaFacts;
use crate::pages::pill::{PillFacts, PillState};
use crate::pages::system::{MicFacts, SystemFacts};

/// The clock every mock panel shows.
pub const CLOCK: &str = "21:47";
/// The identity of the generated cover art.
pub const ART_ID: ArtId = ArtId(0x4D38_3301);
/// Side of the generated cover art in pixels.
pub const ART_SIZE: u32 = 96;
/// Samples per trace: 8 s at 4 samples a second, plus the closing sample.
pub const TRACE_SAMPLES: usize = 33;
/// Where the track was when the media facts were read, in seconds (1:42).
const POSITION_S: f64 = 102.0;
/// The track length in seconds (4:03).
const DURATION_S: f64 = 243.0;
/// The wall time the fixtures are read at: the media stamp and `LiveValues::now`.
const STAMP_S: u64 = 1_763_329_620;
/// The seconds clock `banana_activity` observes at.
const NOW_S: f64 = 100.0;

fn stamp() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(STAMP_S)
}

fn chan(label: &str, gain_db: f32, muted: bool, routes: &[bool]) -> ChannelFacts {
    ChannelFacts {
        label: label.to_string(),
        gain_db,
        muted,
        routes: routes.to_vec(),
    }
}

/// The mock's diagram: two hardware strips (Mikrofon, Hat girişi), two virtual strips
/// (Windows on VAIO, Discord on AUX); buses A1 Logitech G733, A2 Edifier R1280, B1 Discord and
/// an unused B2. Windows and Discord feed A1, the microphone feeds B1, the muted line in feeds
/// A1 (drawn cut). Banana's third hardware strip and A3 are left out, as in the mock.
#[must_use]
pub fn banana() -> AudioFacts {
    let strips = vec![
        chan("Mikrofon", 0.0, false, &[false, false, true, false]),
        chan("Hat giri\u{15f}i", 0.0, true, &[true, false, false, false]),
        chan("Windows", 0.0, false, &[true, false, false, false]),
        chan("Discord", 0.0, false, &[true, false, false, false]),
    ];
    let buses = vec![
        chan("Logitech G733", -6.0, false, &[]),
        chan("Edifier R1280", -14.0, false, &[]),
        chan("Discord", 0.0, false, &[]),
        chan("", 0.0, false, &[]),
    ];
    AudioFacts {
        vm: Some(VmFacts {
            strips,
            buses,
            hardware_strips: 2,
            hardware_buses: 2,
        }),
        ..AudioFacts::default()
    }
}

/// Signal on every strip but the muted line in, and the seconds clock to read it at.
#[must_use]
pub fn banana_activity() -> (Activity, f64) {
    let mut activity = Activity::default();
    let mut levels = [[0.4_f32, 0.35]; 8];
    levels[1] = [0.0, 0.0];
    activity.observe(&levels, NOW_S);
    (activity, NOW_S)
}

/// Voicemeeter Potato: five hardware strips and three virtual ones, five A and three B buses,
/// with plausible labels and a few routes.
#[must_use]
pub fn potato() -> AudioFacts {
    let strip_names = [
        "Mikrofon",
        "Hat giri\u{15f}i",
        "Gitar",
        "Synth",
        "Konsol",
        "Windows",
        "Discord",
        "Oyun",
    ];
    let bus_names = [
        "Logitech G733",
        "Edifier R1280",
        "Focusrite 2i2",
        "",
        "",
        "Discord",
        "OBS",
        "",
    ];
    let routes: [&[bool]; 8] = [
        &[false, false, false, false, false, true, true, false],
        &[false, true, false, false, false, false, false, false],
        &[false, false, true, false, false, false, false, false],
        &[],
        &[],
        &[true, false, false, false, false, false, true, false],
        &[true, false, false, false, false, false, false, false],
        &[true, false, false, false, false, false, true, false],
    ];
    let strips = strip_names
        .iter()
        .zip(routes)
        .map(|(name, r)| chan(name, 0.0, false, r))
        .collect();
    let buses = bus_names
        .iter()
        .map(|name| chan(name, 0.0, false, &[]))
        .collect();
    AudioFacts {
        vm: Some(VmFacts {
            strips,
            buses,
            hardware_strips: 5,
            hardware_buses: 5,
        }),
        ..AudioFacts::default()
    }
}

/// No Voicemeeter: Windows plays to the default G733 at 62; the Edifier is reachable.
#[must_use]
pub fn no_voicemeeter() -> AudioFacts {
    let endpoint = |id: &str, name: &str| EndpointFacts {
        id: id.to_string(),
        name: name.to_string(),
    };
    AudioFacts {
        vm: None,
        endpoints: vec![
            endpoint("g733", "Logitech G733"),
            endpoint("r1280", "Edifier R1280"),
        ],
        default_id: Some("g733".to_string()),
        attached: Some((0.62, false)),
        mic: None,
    }
}

/// Midnight City by M83, playing at 1:42 of 4:03 to A1 G733, with the generated art.
#[must_use]
pub fn media_playing() -> MediaFacts {
    MediaFacts {
        has_session: true,
        source_name: "Spotify".to_string(),
        title: "Midnight City".to_string(),
        artist: "M83 \u{b7} Hurry Up, We\u{2019}re Dreaming".to_string(),
        art: Some(ART_ID),
        playing: true,
        can_seek: true,
        can_next: true,
        can_prev: true,
        timeline: Some(SeekTimeline {
            position_s: POSITION_S,
            duration_s: DURATION_S,
            stamp: stamp(),
            playing: true,
        }),
        output: Some("A1 \u{b7} G733".to_string()),
        reachable: Some("A2 \u{b7} EDIFIER".to_string()),
    }
}

/// Nothing playing; the route still names the output.
#[must_use]
pub fn media_empty() -> MediaFacts {
    MediaFacts {
        output: Some("A1 \u{b7} G733".to_string()),
        ..MediaFacts::default()
    }
}

/// Mixes two 8-bit channels at `t` (0 to 1).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn mix(a: u8, b: u8, t: f64) -> u8 {
    // Both ends are 0..=255 and t is 0..=1, so the result fits a u8.
    (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8
}

/// The mock's cover placeholder: a diagonal gradient from `#3B2F5C` to `#B0476B`, opaque BGRA.
#[must_use]
pub fn art() -> ArtPixels {
    let n = ART_SIZE;
    let mut bgra = Vec::with_capacity((n * n * 4) as usize);
    let span = f64::from(2 * (n - 1));
    for y in 0..n {
        for x in 0..n {
            let t = f64::from(x + y) / span;
            bgra.extend_from_slice(&[
                mix(0x5C, 0x6B, t),
                mix(0x2F, 0x47, t),
                mix(0x3B, 0xB0, t),
                255,
            ]);
        }
    }
    ArtPixels {
        id: ART_ID,
        width: n,
        height: n,
        bgra: Arc::from(bgra),
    }
}

/// Brightness on the DELL AW2725DF at 70; the SHURE MV7 feeds B1, open.
#[must_use]
pub fn system() -> SystemFacts {
    SystemFacts {
        brightness: Some(("DELL AW2725DF".to_string(), 70)),
        mic: Some(MicFacts {
            name: "HW 1 \u{b7} SHURE MV7".to_string(),
            muted: false,
        }),
        mic_bus: Some("B1".to_string()),
    }
}

/// The same with the microphone muted.
#[must_use]
pub fn system_mic_muted() -> SystemFacts {
    let mut facts = system();
    if let Some(mic) = facts.mic.as_mut() {
        mic.muted = true;
    }
    facts
}

/// The same on a monitor without DDC/CI.
#[must_use]
pub fn system_no_ddc() -> SystemFacts {
    SystemFacts {
        brightness: None,
        ..system()
    }
}

/// The three pills of the mock: playing at 62, muted, idle.
#[must_use]
pub fn pills() -> [PillFacts; 3] {
    let pill = |state| PillFacts {
        output: "G733".to_string(),
        value: Some(62),
        state,
    };
    [
        pill(PillState::Live),
        pill(PillState::Muted),
        pill(PillState::Idle),
    ]
}

/// One trace as the mock draws it: a walk of `sin(seed * 7 + i * 1.7) * 0.22` steps from 0.5,
/// held within 0.1 to 0.95.
fn trace(seed: f64) -> TraceSamples {
    let mut v = 0.5_f64;
    let samples = (0..TRACE_SAMPLES)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            // Sample indices are tiny, the cast is exact.
            let i = i as f64;
            v = (v + (seed * 7.0 + i * 1.7).sin() * 0.22).clamp(0.1, 0.95);
            #[allow(clippy::cast_possible_truncation)]
            // The value is within 0.1..0.95.
            let s = v as f32;
            s
        })
        .collect();
    TraceSamples(samples)
}

/// Live values for every fixture: levels and 8 s traces on eight strips, and `now` at the media
/// stamp so the seek head sits at 1:42.
#[must_use]
pub fn live_values() -> LiveValues {
    let traces = (1..=8).map(|seed| trace(f64::from(seed))).collect();
    LiveValues {
        strips: vec![[0.4, 0.35]; 8],
        buses: vec![[0.4, 0.35]; 8],
        traces,
        now: stamp(),
    }
}

#[cfg(test)]
mod tests;
