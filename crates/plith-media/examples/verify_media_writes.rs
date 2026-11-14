//! Transport write verification for plith-media. THIS EXAMPLE CHANGES THE PLAYING MEDIA.
//!
//! Approval requirement: run it with `--yes` only after the owner has approved it. Without
//! `--yes` it reads the current session, prints exactly what it WOULD do and how it restores
//! it, then exits 0 having sent nothing.
//!
//! Section: `--media-transport` (nothing runs without it).
//! - play/pause is toggled twice and must end in the original state, judged by the bools the
//!   calls return and by the session snapshot;
//! - the position is moved +5 s (or -5 s near the end) and then back to the original position
//!   (skipped with a message when there is no session or the source cannot seek).
//!
//! Every change sits behind a restore guard, so a failed step or a panic still puts the
//! original state back. Ctrl+C is turned into a flag: the run falls through to the guards.

#[cfg(windows)]
mod cli {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{Receiver, RecvTimeoutError};
    use std::time::{Duration, Instant, SystemTime};

    use plith_media::{MediaService, MediaSnapshot, Status, position_at};
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    use windows::core::BOOL;

    const EVENT_WAIT: Duration = Duration::from_secs(2);
    const SEEK_STEP: Duration = Duration::from_secs(5);
    const SEEK_TOLERANCE: Duration = Duration::from_secs(2);

    static INTERRUPTED: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn on_ctrl(_kind: u32) -> BOOL {
        INTERRUPTED.store(true, Ordering::SeqCst);
        // Handled: the process keeps running so the guards can restore the media state.
        BOOL(1)
    }

    fn interrupted() -> bool {
        INTERRUPTED.load(Ordering::SeqCst)
    }

    /// Runs `undo` once: from `restore_now`, or on drop (early return or panic unwinding).
    struct Guard<F: FnMut()> {
        undo: F,
        armed: bool,
    }

    impl<F: FnMut()> Guard<F> {
        fn new(undo: F) -> Self {
            Self { undo, armed: true }
        }

        fn restore_now(&mut self) {
            if self.armed {
                self.armed = false;
                (self.undo)();
            }
        }
    }

    impl<F: FnMut()> Drop for Guard<F> {
        fn drop(&mut self) {
            self.restore_now();
        }
    }

    /// Waits for a snapshot that satisfies `wanted`; false on timeout or Ctrl+C.
    fn event_arrived(
        events: &Receiver<MediaSnapshot>,
        wanted: impl Fn(&MediaSnapshot) -> bool,
    ) -> bool {
        let deadline = Instant::now() + EVENT_WAIT;
        while !interrupted() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            match events.recv_timeout(left.min(Duration::from_millis(100))) {
                Ok(snapshot) if wanted(&snapshot) => return true,
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        }
        false
    }

    fn drain(events: &Receiver<MediaSnapshot>) {
        while events.try_recv().is_ok() {}
    }

    fn position_now(snapshot: &MediaSnapshot) -> Option<Duration> {
        let timeline = snapshot.timeline?;
        Some(position_at(&timeline, snapshot.playing, SystemTime::now()))
    }

    fn near(a: Duration, b: Duration) -> bool {
        a.abs_diff(b) <= SEEK_TOLERANCE
    }

    /// The target of the +5 s seek: forward, or backward when that would pass the end.
    pub fn seek_target(original: Duration, duration: Duration) -> Duration {
        let forward = original + SEEK_STEP;
        if forward < duration {
            forward
        } else {
            original.saturating_sub(SEEK_STEP)
        }
    }

    /// Verdict of the two toggles: both accepted, ending in the original playing state.
    pub fn judge_toggles(accepted: [bool; 2], original: bool, now_playing: bool) -> bool {
        accepted == [true, true] && now_playing == original
    }

    /// Tracks how many toggles are believed applied, from the calls' own return values.
    #[derive(Default)]
    struct ToggleTrack {
        applied: Cell<u32>,
        unsure: Cell<bool>,
    }

    impl ToggleTrack {
        fn record(&self, accepted: bool) -> bool {
            if accepted {
                self.applied.set(self.applied.get() + 1);
            } else {
                self.unsure.set(true);
            }
            accepted
        }
    }

    fn transport(
        service: &MediaService,
        before: &MediaSnapshot,
        yes: bool,
    ) -> Result<bool, String> {
        let original = before.playing;
        println!("BEFORE: playing={original} on {}", before.source_id);
        if !yes {
            println!("WOULD: toggle play/pause twice, ending playing={original}");
            return Ok(true);
        }
        let events = service.subscribe();
        let track = ToggleTrack::default();
        let mut guard = Guard::new(|| {
            let odd = track.applied.get() % 2 == 1;
            let off = track.unsure.get() && service.snapshot().playing != original;
            if odd || off {
                let _ = track.record(service.toggle_play_pause());
            }
        });
        drain(&events);
        if interrupted() {
            return Err("interrupted before the write".into());
        }
        let first = track.record(service.toggle_play_pause());
        let arrived = event_arrived(&events, |s| s.playing != original);
        println!(
            "AFTER-CHANGE: accepted={first}, playing={}, event: {arrived}",
            service.snapshot().playing
        );
        drain(&events);
        let second = track.record(service.toggle_play_pause());
        guard.restore_now();
        let arrived = event_arrived(&events, |s| s.playing == original);
        let now_playing = service.snapshot().playing;
        println!(
            "AFTER-RESTORE: accepted={second}, playing={now_playing} (original {original}), event: {arrived}"
        );
        Ok(judge_toggles([first, second], original, now_playing))
    }

    fn seeking(service: &MediaService, before: &MediaSnapshot, yes: bool) -> Result<bool, String> {
        let Some(timeline) = before.timeline.filter(|_| before.can_seek) else {
            return Err("the source cannot seek (or has no timeline)".into());
        };
        let original = position_now(before).unwrap_or(timeline.position);
        let target = seek_target(original, timeline.duration);
        println!(
            "BEFORE: position {:.1}s of {:.1}s",
            original.as_secs_f64(),
            timeline.duration.as_secs_f64()
        );
        if !yes {
            println!(
                "WOULD: seek to {:.1}s, then back to {:.1}s",
                target.as_secs_f64(),
                original.as_secs_f64()
            );
            return Ok(true);
        }
        let events = service.subscribe();
        let moved = Cell::new(false);
        let mut guard = Guard::new(|| {
            if moved.replace(false) {
                let _ = service.seek(original);
            }
        });
        drain(&events);
        if interrupted() {
            return Err("interrupted before the write".into());
        }
        moved.set(true);
        let first = service.seek(target);
        let at = |want: Duration| {
            move |s: &MediaSnapshot| position_now(s).is_some_and(|p| near(p, want))
        };
        let arrived = event_arrived(&events, at(target));
        let after = position_now(&service.snapshot());
        println!(
            "AFTER-CHANGE: accepted={first}, position {after:?} (wanted {target:?}), event: {arrived}"
        );
        let changed = first && after.is_some_and(|p| near(p, target));
        drain(&events);
        let second = service.seek(original);
        moved.set(!second);
        guard.restore_now();
        let arrived = event_arrived(&events, at(original));
        let restored = position_now(&service.snapshot());
        println!(
            "AFTER-RESTORE: accepted={second}, position {restored:?} (original {original:?}), event: {arrived}"
        );
        Ok(changed && second && restored.is_some_and(|p| near(p, original)))
    }

    fn report(name: &str, outcome: Result<bool, String>) -> bool {
        match outcome {
            Ok(true) => println!("{name}: PASS (or planned only)"),
            Ok(false) => println!("{name}: FAIL"),
            Err(why) => {
                println!("{name}: SKIPPED ({why})");
                return true;
            }
        }
        matches!(outcome, Ok(true))
    }

    pub fn run() -> bool {
        let mut yes = false;
        let mut transport_wanted = false;
        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "--yes" => yes = true,
                "--media-transport" => transport_wanted = true,
                other => {
                    eprintln!("unknown argument {other}");
                    std::process::exit(2);
                }
            }
        }
        if !transport_wanted {
            println!("no section selected (use --media-transport), nothing to do");
            return true;
        }
        let service = MediaService::new();
        if let Status::Unavailable(reason) = service.start() {
            println!("media-transport: SKIPPED (unavailable: {reason})");
            return true;
        }
        let before = service.snapshot();
        if !before.has_session {
            println!("media-transport: SKIPPED (no media session)");
            return true;
        }
        if yes {
            println!("WRITE MODE (--yes): play/pause and position are changed and restored");
            println!("do not press Ctrl+C; if it is pressed the run restores the state and stops");
            // SAFETY: `on_ctrl` is a valid handler routine that only stores to a static atomic.
            if unsafe { SetConsoleCtrlHandler(Some(on_ctrl), true) }.is_err() {
                println!("warning: could not install the Ctrl+C handler");
            }
        } else {
            println!("DRY PLAN (no --yes): nothing is sent");
        }
        let mut ok = report(
            "media-transport (play/pause)",
            transport(&service, &before, yes),
        );
        if !interrupted() {
            let latest = service.snapshot();
            ok &= report("media-transport (seek)", seeking(&service, &latest, yes));
        }
        service.stop();
        ok
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn toggles_must_both_be_accepted_and_end_original() {
            assert!(judge_toggles([true, true], true, true));
            assert!(!judge_toggles([true, false], true, true));
            assert!(!judge_toggles([true, true], true, false));
        }

        #[test]
        fn seek_goes_back_near_the_end() {
            let long = Duration::from_secs(200);
            assert_eq!(
                seek_target(Duration::from_secs(10), long),
                Duration::from_secs(15)
            );
            assert_eq!(
                seek_target(Duration::from_secs(197), long),
                Duration::from_secs(192)
            );
            assert_eq!(
                seek_target(Duration::from_secs(2), Duration::from_secs(4)),
                Duration::ZERO
            );
        }
    }
}

#[cfg(windows)]
fn main() {
    if !cli::run() {
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    println!("SMTC is Windows only");
}
