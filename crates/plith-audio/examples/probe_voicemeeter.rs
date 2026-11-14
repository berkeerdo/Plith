//! Read-only probe: prints the Voicemeeter edition, every strip and bus, then levels for 3 s.
//!
//! It never calls `set_gain` or `toggle_mute`. If Voicemeeter is not installed it reports
//! `Unavailable` and exits.

#[cfg(windows)]
fn main() {
    use plith_audio::voicemeeter::{Status, VoicemeeterService, level_to_db};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    let service = VoicemeeterService::new();
    let status = service.start();
    println!("status: {status:?}");
    if matches!(status, Status::Unavailable(_)) {
        return;
    }

    let deadline = Instant::now() + Duration::from_secs(15);
    let snapshot = loop {
        if let Some(s) = service.snapshot() {
            break s;
        }
        if Instant::now() > deadline {
            println!("no snapshot within 15 s (engine still starting?)");
            service.stop();
            return;
        }
        sleep(Duration::from_millis(100));
    };

    println!("type: {:?}", snapshot.kind);
    for ch in snapshot.strips.iter().chain(&snapshot.buses) {
        println!(
            "{:?}[{}] label={:?} gain={:.1} dB muted={}",
            ch.rail, ch.index, ch.label, ch.gain_db, ch.muted
        );
    }

    println!("levels (peak dB, L/R) for 3 s:");
    for step in 0..6 {
        sleep(Duration::from_millis(500));
        let levels = service.levels();
        let fmt = |rows: &[[f32; 2]]| {
            rows.iter()
                .map(|[l, r]| format!("{:.0}/{:.0}", level_to_db(*l), level_to_db(*r)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!(
            "t={:.1}s strips: {} | buses: {}",
            f64::from(step + 1) * 0.5,
            fmt(&levels.strips),
            fmt(&levels.buses)
        );
    }
    service.stop();
    println!("stopped");
}

#[cfg(not(windows))]
fn main() {
    println!("Voicemeeter is Windows only");
}
