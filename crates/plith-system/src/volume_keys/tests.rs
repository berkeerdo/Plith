//! The hook proc's decision is pure, so what it does (and does not do) is asserted here.

use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::{Duration, Instant};

use super::{
    HeldKeys, VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP, VolumeKey, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP, handle_key_event as handle,
};
use crate::flyout::SuppressionWindow;

#[test]
fn volume_keys_only_enqueue() {
    let (tx, rx) = sync_channel(8);
    for vk in [VK_VOLUME_MUTE, VK_VOLUME_DOWN, VK_VOLUME_UP] {
        handle(
            0,
            WM_KEYDOWN,
            vk,
            &mut HeldKeys::default(),
            &tx,
            None,
            Instant::now,
        );
    }
    handle(
        0,
        WM_SYSKEYDOWN,
        VK_VOLUME_UP,
        &mut HeldKeys::default(),
        &tx,
        None,
        Instant::now,
    );
    assert_eq!(rx.try_iter().count(), 4);
}

#[test]
fn other_input_enqueues_nothing() {
    let (tx, rx) = sync_channel(8);
    handle(
        0,
        WM_KEYDOWN,
        0x41,
        &mut HeldKeys::default(),
        &tx,
        None,
        Instant::now,
    );
    handle(
        0,
        0x0101,
        VK_VOLUME_UP,
        &mut HeldKeys::default(),
        &tx,
        None,
        Instant::now,
    );
    handle(
        -1,
        WM_KEYDOWN,
        VK_VOLUME_UP,
        &mut HeldKeys::default(),
        &tx,
        None,
        Instant::now,
    );
    assert_eq!(rx.try_iter().count(), 0);
}

#[test]
fn never_blocks_when_full() {
    let (tx, rx) = sync_channel(1);
    for _ in 0..1000 {
        handle(
            0,
            WM_KEYDOWN,
            VK_VOLUME_UP,
            &mut HeldKeys::default(),
            &tx,
            None,
            Instant::now,
        );
    }
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn never_blocks_when_receiver_gone() {
    let (tx, rx) = sync_channel(1);
    drop(rx);
    for _ in 0..1000 {
        handle(
            0,
            WM_KEYDOWN,
            VK_VOLUME_UP,
            &mut HeldKeys::default(),
            &tx,
            None,
            Instant::now,
        );
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
        handle(
            0,
            WM_KEYDOWN,
            std::hint::black_box(vk),
            &mut HeldKeys::default(),
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
    handle(
        0,
        WM_KEYDOWN,
        VK_VOLUME_UP,
        &mut HeldKeys::default(),
        &tx,
        Some(&window),
        || t0,
    );
    assert!(window.is_open(t0 + Duration::from_millis(399)));
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn other_input_leaves_the_window_closed_and_skips_the_clock() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, rx) = sync_channel(8);
    let clock = || panic!("the clock must only be read for a volume key");
    handle(
        0,
        WM_KEYDOWN,
        0x41,
        &mut HeldKeys::default(),
        &tx,
        Some(&window),
        clock,
    );
    handle(
        -1,
        WM_KEYDOWN,
        VK_VOLUME_UP,
        &mut HeldKeys::default(),
        &tx,
        Some(&window),
        clock,
    );
    assert!(!window.is_open(t0));
    assert_eq!(rx.try_iter().count(), 0);
}

#[test]
fn window_still_opens_when_the_queue_is_full() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, _rx) = sync_channel(1);
    for _ in 0..10 {
        handle(
            0,
            WM_KEYDOWN,
            VK_VOLUME_UP,
            &mut HeldKeys::default(),
            &tx,
            Some(&window),
            || t0,
        );
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
        handle(
            0,
            WM_KEYDOWN,
            vk,
            &mut HeldKeys::default(),
            &tx,
            Some(&window),
            Instant::now,
        );
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
            Ok(_) => {}
            Err(err) => break err,
        }
    };
    assert_eq!(end, TryRecvError::Disconnected);
}

fn down(
    key: u32,
    held: &mut HeldKeys,
    tx: &SyncSender<VolumeKey>,
    win: Option<&SuppressionWindow>,
    at: Instant,
) {
    handle(0, WM_KEYDOWN, key, held, tx, win, || at);
}

#[test]
fn held_key_sends_once_for_135_downs() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, rx) = sync_channel(8);
    let mut held = HeldKeys::default();
    let mut opened = 0;
    for i in 0..135u64 {
        let at = t0 + Duration::from_millis(30 * i);
        down(VK_VOLUME_UP, &mut held, &tx, Some(&window), at);
        // Each down must have (re)opened the window from its own instant.
        if window.is_open(at + Duration::from_millis(399)) {
            opened += 1;
        }
    }
    handle(
        0,
        WM_KEYUP,
        VK_VOLUME_UP,
        &mut held,
        &tx,
        Some(&window),
        || panic!("no clock on up"),
    );
    assert_eq!(opened, 135);
    assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![VolumeKey::Up]);
}

#[test]
fn release_then_press_sends_again() {
    let t0 = Instant::now();
    let (tx, rx) = sync_channel(8);
    let mut held = HeldKeys::default();
    down(VK_VOLUME_DOWN, &mut held, &tx, None, t0);
    handle(0, WM_KEYUP, VK_VOLUME_DOWN, &mut held, &tx, None, || t0);
    down(
        VK_VOLUME_DOWN,
        &mut held,
        &tx,
        None,
        t0 + Duration::from_millis(100),
    );
    handle(0, WM_SYSKEYUP, VK_VOLUME_DOWN, &mut held, &tx, None, || t0);
    down(
        VK_VOLUME_DOWN,
        &mut held,
        &tx,
        None,
        t0 + Duration::from_millis(200),
    );
    assert_eq!(rx.try_iter().count(), 3);
}

#[test]
fn keys_are_independent() {
    let t0 = Instant::now();
    let (tx, rx) = sync_channel(8);
    let mut held = HeldKeys::default();
    down(VK_VOLUME_UP, &mut held, &tx, None, t0);
    down(
        VK_VOLUME_DOWN,
        &mut held,
        &tx,
        None,
        t0 + Duration::from_millis(10),
    );
    down(
        VK_VOLUME_UP,
        &mut held,
        &tx,
        None,
        t0 + Duration::from_millis(40),
    );
    assert_eq!(
        rx.try_iter().collect::<Vec<_>>(),
        vec![VolumeKey::Up, VolumeKey::Down]
    );
}

#[test]
fn up_without_down_is_harmless() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let (tx, rx) = sync_channel(8);
    let mut held = HeldKeys::default();
    let clock = || panic!("the clock is not needed on a key up");
    handle(
        0,
        WM_KEYUP,
        VK_VOLUME_MUTE,
        &mut held,
        &tx,
        Some(&window),
        clock,
    );
    handle(
        0,
        WM_SYSKEYUP,
        VK_VOLUME_MUTE,
        &mut held,
        &tx,
        Some(&window),
        clock,
    );
    assert!(!window.is_open(t0));
    assert_eq!(rx.try_iter().count(), 0);
    down(VK_VOLUME_MUTE, &mut held, &tx, None, t0);
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn stale_hold_counts_as_new_press() {
    let t0 = Instant::now();
    let (tx, rx) = sync_channel(8);
    let mut held = HeldKeys::default();
    down(VK_VOLUME_UP, &mut held, &tx, None, t0);
    // The key up was lost (hook timeout): a down 1.6 s later is a new press.
    down(
        VK_VOLUME_UP,
        &mut held,
        &tx,
        None,
        t0 + Duration::from_millis(1600),
    );
    assert_eq!(rx.try_iter().count(), 2);
}
