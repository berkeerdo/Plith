//! The hook proc's decision is pure, so what it does (and does not do) is asserted here.

use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use super::handle_key_event;
use super::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP, WM_KEYDOWN, WM_SYSKEYDOWN};
use crate::flyout::SuppressionWindow;

#[test]
fn volume_keys_only_enqueue() {
    let (tx, rx) = sync_channel(8);
    for vk in [VK_VOLUME_MUTE, VK_VOLUME_DOWN, VK_VOLUME_UP] {
        handle_key_event(0, WM_KEYDOWN, vk, &tx, None, Instant::now);
    }
    handle_key_event(0, WM_SYSKEYDOWN, VK_VOLUME_UP, &tx, None, Instant::now);
    assert_eq!(rx.try_iter().count(), 4);
}

#[test]
fn other_input_enqueues_nothing() {
    let (tx, rx) = sync_channel(8);
    handle_key_event(0, WM_KEYDOWN, 0x41, &tx, None, Instant::now);
    handle_key_event(0, 0x0101, VK_VOLUME_UP, &tx, None, Instant::now);
    handle_key_event(-1, WM_KEYDOWN, VK_VOLUME_UP, &tx, None, Instant::now);
    assert_eq!(rx.try_iter().count(), 0);
}

#[test]
fn never_blocks_when_full() {
    let (tx, rx) = sync_channel(1);
    for _ in 0..1000 {
        handle_key_event(0, WM_KEYDOWN, VK_VOLUME_UP, &tx, None, Instant::now);
    }
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn never_blocks_when_receiver_gone() {
    let (tx, rx) = sync_channel(1);
    drop(rx);
    for _ in 0..1000 {
        handle_key_event(0, WM_KEYDOWN, VK_VOLUME_UP, &tx, None, Instant::now);
    }
}

#[test]
fn handler_is_cheap_per_call() {
    const CALLS: u32 = 100_000;
    let (tx, _rx) = sync_channel(1);
    let started = Instant::now();
    for i in 0..CALLS {
        // Mostly non-matching keys (the real distribution), plus volume keys into a full channel.
        let vk = if i % 10 == 0 { VK_VOLUME_UP } else { 0x41 };
        handle_key_event(
            0,
            WM_KEYDOWN,
            std::hint::black_box(vk),
            &tx,
            None,
            Instant::now,
        );
    }
    let mean = started.elapsed() / CALLS;
    println!("mean per call: {mean:?}");
    assert!(mean.as_micros() < 5, "mean {mean:?} per call");
}

#[test]
fn volume_key_opens_the_window_inline() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, rx) = sync_channel(8);
    handle_key_event(0, WM_KEYDOWN, VK_VOLUME_UP, &tx, Some(&window), || t0);
    assert!(window.is_open(t0 + Duration::from_millis(399)));
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn other_input_leaves_the_window_closed_and_skips_the_clock() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, rx) = sync_channel(8);
    let clock = || panic!("the clock must only be read for a volume key");
    handle_key_event(0, WM_KEYDOWN, 0x41, &tx, Some(&window), clock);
    handle_key_event(-1, WM_KEYDOWN, VK_VOLUME_UP, &tx, Some(&window), clock);
    assert!(!window.is_open(t0));
    assert_eq!(rx.try_iter().count(), 0);
}

#[test]
fn window_still_opens_when_the_queue_is_full() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, _rx) = sync_channel(1);
    for _ in 0..10 {
        handle_key_event(0, WM_KEYDOWN, VK_VOLUME_UP, &tx, Some(&window), || t0);
    }
    assert!(window.is_open(t0));
}

#[test]
fn handler_with_a_window_is_still_cheap_per_call() {
    const CALLS: u32 = 100_000;
    let window = SuppressionWindow::new(Instant::now());
    let (tx, _rx) = sync_channel(1);
    let started = Instant::now();
    for i in 0..CALLS {
        // Every tenth key is a volume key: the window store plus a send into a full channel.
        let vk = if i % 10 == 0 { VK_VOLUME_UP } else { 0x41 };
        let vk = std::hint::black_box(vk);
        handle_key_event(0, WM_KEYDOWN, vk, &tx, Some(&window), Instant::now);
    }
    let mean = started.elapsed() / CALLS;
    println!("mean per call with a window: {mean:?}");
    assert!(mean.as_micros() < 5, "mean {mean:?} per call");
}

/// Installs the hook, stops it and proves the thread joined (stop is `Ok`) and the receiver
/// disconnected. Only listens; it injects no keys.
#[cfg(windows)]
#[test]
fn start_stop_joins_and_disconnects() {
    use std::sync::mpsc::TryRecvError;

    use super::VolumeKeyHook;

    let (mut hook, rx) = VolumeKeyHook::start(None).expect("hook installs");
    assert_eq!(hook.stop(), Ok(()));
    assert_eq!(hook.stop(), Ok(()), "stopping twice is fine");
    let end = loop {
        match rx.try_recv() {
            Ok(()) => {}
            Err(err) => break err,
        }
    };
    assert_eq!(end, TryRecvError::Disconnected);
}
