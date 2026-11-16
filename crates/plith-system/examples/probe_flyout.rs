//! Dry run probe: installs the `WinEvent` hooks and the volume key hook for 10 s, logs what the
//! suppressor WOULD hide when a volume key is pressed by hand, hides nothing, then unhooks.
//! Both owners live in this scope, so they unhook on every exit path. No key or mouse input is
//! sent by this program.
//!
//! `--brightness` instead starts the brightness service and the suppressor (dry run), wires the
//! suppressor's window into the service, and for 10 s prints every window that would be hidden
//! and the listener error. It never calls `set_percent`. Only a built-in panel raises
//! brightness events, so a desktop prints none.

#[cfg(windows)]
use std::sync::mpsc::RecvTimeoutError;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
use plith_system::{BrightnessService, FlyoutSuppressor, Mode, VolumeKeyHook};

#[cfg(windows)]
fn brightness_probe() {
    let suppressor = match FlyoutSuppressor::start(Mode::DryRun) {
        Ok(s) => s,
        Err(why) => {
            println!("suppressor unavailable: {why}");
            return;
        }
    };
    let service = BrightnessService::new();
    println!("brightness service: {:?}", service.start());
    service.set_suppression_window(Some(suppressor.suppression_window()));
    println!("wired (dry run, nothing is hidden); change brightness by hand within 10 s");
    let started = Instant::now();
    let mut seen = 0;
    while started.elapsed() < Duration::from_secs(10) {
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
        std::thread::sleep(Duration::from_millis(5));
    }
    println!("{seen} windows would be hidden");
    println!("listener error: {:?}", service.listener_error());
    service.stop();
    drop(suppressor);
    println!("stopped");
}

#[cfg(windows)]
fn main() {
    if std::env::args().any(|arg| arg == "--brightness") {
        brightness_probe();
        return;
    }
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
            Ok(key) => {
                presses += 1;
                println!(
                    "[{:5.2}s] volume key {key:?} (the hook opened the window inline)",
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
