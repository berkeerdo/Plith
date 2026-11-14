//! Dry run probe: installs the `WinEvent` hooks and the volume key hook for 10 s, logs what the
//! suppressor WOULD hide when a volume key is pressed by hand, hides nothing, then unhooks.
//! Both owners live in this scope, so they unhook on every exit path. No key or mouse input is
//! sent by this program.

#[cfg(windows)]
fn main() {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant};

    use plith_system::{FlyoutSuppressor, Mode, VolumeKeyHook};

    let suppressor = match FlyoutSuppressor::start(Mode::DryRun) {
        Ok(s) => s,
        Err(why) => {
            println!("suppressor unavailable: {why}");
            return;
        }
    };
    let (keys_owner, keys) = match VolumeKeyHook::start(Some(suppressor.suppression_window())) {
        Ok(pair) => pair,
        Err(why) => {
            println!("key hook unavailable: {why}");
            return;
        }
    };
    println!("hooks installed (dry run, nothing is hidden); press a volume key within 10 s");

    let started = Instant::now();
    let (mut presses, mut seen) = (0, 0);
    while started.elapsed() < Duration::from_secs(10) {
        match keys.recv_timeout(Duration::from_millis(5)) {
            Ok(()) => {
                presses += 1;
                println!(
                    "[{:5.2}s] volume key (the hook opened the window inline)",
                    started.elapsed().as_secs_f64()
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        for m in suppressor.matches().try_iter() {
            seen += 1;
            println!(
                "[{:5.2}s] WOULD hide hwnd=0x{:X} class='{}' process='{}' band={:?}",
                started.elapsed().as_secs_f64(),
                m.hwnd,
                m.facts.class,
                m.facts.process,
                m.facts.band
            );
        }
    }
    println!("{presses} volume key presses, {seen} windows would be hidden");
    drop(keys_owner);
    drop(suppressor);
    println!("unhooked");
}

#[cfg(not(windows))]
fn main() {
    println!("flyout suppression is Windows only");
}
