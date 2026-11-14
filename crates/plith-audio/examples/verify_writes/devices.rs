//! Microphone mute and Voicemeeter gain sections.

use std::time::{Duration, Instant};

use plith_audio::microphone::MicrophoneService;
use plith_audio::voicemeeter::{self, Rail, VmSnapshot, VoicemeeterService};

use crate::support::{Guard, Verdict, close, drain, event_arrived, judge, planned, yes_no};

const GAIN_STEP_DB: f32 = 0.5;
const GAIN_TOLERANCE_DB: f32 = 0.01;

pub fn mic_mute(service: &MicrophoneService, yes: bool) -> Verdict {
    let Some(before) = service.snapshot() else {
        return Verdict::Skipped("no capture device".into());
    };
    let original = before.muted;
    println!(
        "BEFORE: muted={original} on {} ({})",
        before.name, before.id
    );
    if !yes {
        return planned(&format!(
            "toggle microphone mute twice, ending muted={original}"
        ));
    }
    let events = service.subscribe();
    let muted_now = || service.snapshot().map(|m| m.muted);
    let mut guard = Guard::new(|| {
        if muted_now().is_some_and(|m| m != original) {
            let _ = service.toggle_mute();
        }
    });
    drain(&events);
    let toggled = service.toggle_mute();
    let arrived = event_arrived(&events, |s| s.as_ref().is_some_and(|m| m.muted != original));
    println!(
        "AFTER-CHANGE: toggle returned {toggled:?}, muted={:?} (wanted {}), event: {}",
        muted_now(),
        !original,
        yes_no(arrived)
    );
    let changed = muted_now() == Some(!original);
    drain(&events);
    guard.restore_now();
    let arrived = event_arrived(&events, |s| s.as_ref().is_some_and(|m| m.muted == original));
    println!(
        "AFTER-RESTORE: muted={:?} (original {original}), event: {}",
        muted_now(),
        yes_no(arrived)
    );
    judge(changed, muted_now() == Some(original))
}

pub fn wait_snapshot(service: &VoicemeeterService) -> Option<VmSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Some(snapshot) = service.snapshot() {
            return Some(snapshot);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

pub fn voicemeeter_gain(service: &VoicemeeterService, yes: bool) -> Verdict {
    if let voicemeeter::Status::Unavailable(reason) = service.start() {
        return Verdict::Skipped(format!("unavailable: {reason}"));
    }
    let Some(before) = wait_snapshot(service) else {
        return Verdict::Skipped("unavailable: no snapshot within 15 s".into());
    };
    let Some(strip) = before.strips.first() else {
        return Verdict::Skipped("unavailable: no strip 0".into());
    };
    let original = strip.gain_db;
    let target = if original + GAIN_STEP_DB <= voicemeeter::MAX_GAIN_DB {
        original + GAIN_STEP_DB
    } else {
        original - GAIN_STEP_DB
    };
    println!("BEFORE: strip 0 ({}) gain {original:.2} dB", strip.label);
    if !yes {
        return planned(&format!(
            "set strip 0 gain {target:.2} dB, then restore {original:.2} dB"
        ));
    }
    let events = service.subscribe();
    let gain_now = || {
        service
            .snapshot()
            .and_then(|s| s.strips.first().map(|c| c.gain_db))
    };
    let mut guard = Guard::new(|| {
        let _ = service.set_gain(Rail::Strip, 0, original);
    });
    drain(&events);
    let written = service.set_gain(Rail::Strip, 0, target);
    let hit = |want: f32| {
        move |s: &Option<VmSnapshot>| {
            s.as_ref()
                .and_then(|s| s.strips.first())
                .is_some_and(|c| close(c.gain_db, want, GAIN_TOLERANCE_DB))
        }
    };
    let arrived = event_arrived(&events, hit(target));
    println!(
        "AFTER-CHANGE: write={written} gain {:?} dB (wanted {target:.2}), event: {}",
        gain_now(),
        yes_no(arrived)
    );
    let changed = gain_now().is_some_and(|g| close(g, target, GAIN_TOLERANCE_DB));
    drain(&events);
    guard.restore_now();
    let arrived = event_arrived(&events, hit(original));
    println!(
        "AFTER-RESTORE: gain {:?} dB (original {original:.2}), event: {}",
        gain_now(),
        yes_no(arrived)
    );
    judge(
        changed,
        gain_now().is_some_and(|g| close(g, original, GAIN_TOLERANCE_DB)),
    )
}
