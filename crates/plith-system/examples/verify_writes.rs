//! Hardware write verification for plith-system. THIS EXAMPLE CHANGES THE MACHINE.
//!
//! Approval requirement: run it with `--yes` only after the owner has approved it. Without
//! `--yes` it reads the current values, prints exactly what it WOULD change and how it
//! restores it, then exits 0 having written nothing (no hook is installed either).
//!
//! Sections (none selected with `--yes` does nothing):
//! `--brightness` every display goes to the first display's level +5 percent (clamped; -5 at
//!                100), the coalescing writer is awaited, the level is re-read from the
//!                hardware with a fresh service, then the original level is written back and
//!                re-read. Refused displays are reported per device. Skipped when displays sit
//!                at different levels, because one write sets all of them and a restore would
//!                not be exact.
//! `--flyout`     runs the flyout suppressor in Hide mode for 20 s. Press a volume key once by
//!                hand (this program never sends a key or mouse event). Every window it hid is
//!                printed; after the stop each one is checked to be visible again. A volume key
//!                press changes the volume by one step: press the opposite key afterwards.
//!
//! The original brightness is restored from a drop guard, so a failed step or a panic still
//! writes it back; the suppressor restores its windows when it is dropped.

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use plith_system::{
        BrightnessService, DeviceLevel, FlyoutSuppressor, Mode, Status, VolumeKeyHook,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, IsWindowVisible};

    const WRITE_WAIT: Duration = Duration::from_secs(5);
    const FLYOUT_RUN: Duration = Duration::from_secs(20);
    const BRIGHTNESS_STEP: u8 = 5;
    /// DDC raw values do not always round trip to the same percent.
    const PERCENT_TOLERANCE: u8 = 1;

    enum Verdict {
        Pass,
        Fail(String),
        Skipped(String),
        Planned,
    }

    struct Args {
        yes: bool,
        brightness: bool,
        flyout: bool,
    }

    fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
        let mut out = Args {
            yes: false,
            brightness: false,
            flyout: false,
        };
        for arg in args.skip(1) {
            match arg.as_str() {
                "--yes" => out.yes = true,
                "--brightness" => out.brightness = true,
                "--flyout" => out.flyout = true,
                other => return Err(format!("unknown argument {other}")),
            }
        }
        Ok(out)
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

    fn near(level: u8, want: u8) -> bool {
        level.abs_diff(want) <= PERCENT_TOLERANCE
    }

    fn show(levels: &[DeviceLevel]) -> String {
        levels
            .iter()
            .map(|l| format!("{} ({}) {}%", l.name, l.id, l.percent))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Polls the service snapshot until every display is near `want` or the wait runs out.
    fn wait_levels(service: &BrightnessService, want: u8) {
        let deadline = Instant::now() + WRITE_WAIT;
        while Instant::now() < deadline {
            let levels = service.snapshot();
            if !levels.is_empty() && levels.iter().all(|l| near(l.percent, want)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Reads the levels from the hardware with a service of its own (a fresh discovery), so the
    /// result is not the writer's own bookkeeping.
    fn hardware_levels() -> Vec<DeviceLevel> {
        let fresh = BrightnessService::new();
        match fresh.start() {
            Status::Running => fresh.snapshot(),
            Status::Unavailable(_) => Vec::new(),
        }
    }

    /// Ids of the displays that did not reach `want`.
    fn refused(levels: &[DeviceLevel], want: u8) -> Vec<String> {
        levels
            .iter()
            .filter(|l| !near(l.percent, want))
            .map(|l| l.id.clone())
            .collect()
    }

    fn brightness(yes: bool) -> Verdict {
        let service = BrightnessService::new();
        if let Status::Unavailable(reason) = service.start() {
            return Verdict::Skipped(format!("unavailable: {reason}"));
        }
        let before = service.snapshot();
        let Some(first) = before.first() else {
            return Verdict::Skipped("no controllable display".into());
        };
        println!("BEFORE: {}", show(&before));
        let original = first.percent;
        if before.iter().any(|l| l.percent != original) {
            return Verdict::Skipped(
                "displays are at different levels; one write sets all of them".into(),
            );
        }
        let target = if original + BRIGHTNESS_STEP <= 100 {
            original + BRIGHTNESS_STEP
        } else {
            original - BRIGHTNESS_STEP
        };
        if !yes {
            println!("WOULD: set every display to {target}%, then back to {original}%");
            return Verdict::Planned;
        }
        let mut guard = Guard::new(|| {
            let _ = service.set_percent(original);
            wait_levels(&service, original);
        });
        let accepted = service.set_percent(target);
        wait_levels(&service, target);
        let after = hardware_levels();
        println!(
            "AFTER-CHANGE: accepted={accepted} {} (wanted {target}%)",
            show(&after)
        );
        let refusals = refused(&after, target);
        for id in &refusals {
            println!("  REFUSED or unchanged: {id}");
        }
        guard.restore_now();
        let restored = hardware_levels();
        println!("AFTER-RESTORE: {} (original {original}%)", show(&restored));
        if restored.is_empty() || !refused(&restored, original).is_empty() {
            return Verdict::Fail("the original level was NOT restored".into());
        }
        if after.is_empty() || !refusals.is_empty() {
            return Verdict::Fail(format!("displays did not change: {refusals:?}"));
        }
        Verdict::Pass
    }

    /// Visible, hidden or gone, for a window handle held as an integer.
    fn window_state(hwnd: isize) -> &'static str {
        let handle = HWND(hwnd as *mut _);
        // SAFETY: both calls only query a handle and accept any value, a stale one included.
        unsafe {
            if !IsWindow(Some(handle)).as_bool() {
                "gone"
            } else if IsWindowVisible(handle).as_bool() {
                "visible"
            } else {
                "STILL HIDDEN"
            }
        }
    }

    fn flyout(yes: bool) -> Verdict {
        if !yes {
            println!(
                "WOULD: hook the shell for 20 s in Hide mode, hide the volume flyout when you \
                 press a volume key by hand, then unhook and check every hidden window is visible again"
            );
            return Verdict::Planned;
        }
        let mut suppressor = match FlyoutSuppressor::start(Mode::Hide) {
            Ok(s) => s,
            Err(why) => return Verdict::Skipped(format!("suppressor unavailable: {why}")),
        };
        let (keys_owner, keys) = match VolumeKeyHook::start() {
            Ok(pair) => pair,
            Err(why) => return Verdict::Skipped(format!("key hook unavailable: {why}")),
        };
        println!("hooks installed for 20 s: press a volume key ONCE by hand now");
        let started = Instant::now();
        let (mut presses, mut hidden) = (0, Vec::new());
        let mut reported = 0;
        while started.elapsed() < FLYOUT_RUN {
            if keys.recv_timeout(Duration::from_millis(5)).is_ok() {
                presses += 1;
                suppressor.open_window();
                println!("[{:5.2}s] volume key seen", started.elapsed().as_secs_f64());
            }
            for m in suppressor.matches().try_iter() {
                reported += 1;
                println!(
                    "[{:5.2}s] hwnd=0x{:X} class='{}' process='{}' band={:?} hidden={}",
                    started.elapsed().as_secs_f64(),
                    m.hwnd,
                    m.facts.class,
                    m.facts.process,
                    m.facts.band,
                    m.hidden
                );
                if m.hidden {
                    hidden.push(m.hwnd);
                }
            }
        }
        drop(keys_owner);
        let stopped = suppressor.stop();
        println!("stop: {stopped:?}");
        let mut still_hidden = 0;
        for hwnd in &hidden {
            let state = window_state(*hwnd);
            still_hidden += usize::from(state == "STILL HIDDEN");
            println!("  after stop 0x{hwnd:X}: {state}");
        }
        println!(
            "counts: {presses} key presses, {reported} windows matched, {} hidden, {still_hidden} still hidden",
            hidden.len()
        );
        if let Err(why) = stopped {
            return Verdict::Fail(format!("stop failed: {why}"));
        }
        if still_hidden > 0 {
            return Verdict::Fail(format!("{still_hidden} windows were NOT restored"));
        }
        match (presses, hidden.len()) {
            (0, _) => Verdict::Skipped("no volume key was pressed".into()),
            (_, 0) => Verdict::Fail("key pressed but no window was hidden".into()),
            _ => Verdict::Pass,
        }
    }

    pub(super) fn run() -> bool {
        let args = match parse(std::env::args()) {
            Ok(args) => args,
            Err(why) => {
                eprintln!("{why}");
                std::process::exit(2);
            }
        };
        if !args.brightness && !args.flyout {
            println!("no section selected, nothing to do");
            return true;
        }
        println!(
            "{}",
            if args.yes {
                "WRITE MODE (--yes)"
            } else {
                "DRY PLAN (no --yes): nothing is written"
            }
        );
        let mut results = Vec::new();
        if args.brightness {
            println!("\n== brightness ==");
            results.push(("brightness", brightness(args.yes)));
        }
        if args.flyout {
            println!("\n== flyout ==");
            results.push(("flyout", flyout(args.yes)));
        }
        println!("\n== summary ==");
        let mut ok = true;
        for (name, verdict) in &results {
            match verdict {
                Verdict::Pass => println!("{name}: PASS"),
                Verdict::Fail(why) => {
                    ok = false;
                    println!("{name}: FAIL ({why})");
                }
                Verdict::Skipped(why) => println!("{name}: SKIPPED ({why})"),
                Verdict::Planned => println!("{name}: planned only"),
            }
        }
        ok
    }
}

#[cfg(windows)]
fn main() {
    if !imp::run() {
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    println!("display brightness and flyouts are Windows only");
}
