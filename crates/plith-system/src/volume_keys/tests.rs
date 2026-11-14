//! The hook proc's decision is pure, so what it does (and does not do) is asserted here.

use std::sync::mpsc::sync_channel;
use std::time::Instant;

use super::handle_key_event;
use super::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP, WM_KEYDOWN, WM_SYSKEYDOWN};

#[test]
fn volume_keys_only_enqueue() {
    let (tx, rx) = sync_channel(8);
    for vk in [VK_VOLUME_MUTE, VK_VOLUME_DOWN, VK_VOLUME_UP] {
        handle_key_event(0, WM_KEYDOWN, vk, &tx);
    }
    handle_key_event(0, WM_SYSKEYDOWN, VK_VOLUME_UP, &tx);
    assert_eq!(rx.try_iter().count(), 4);
}

#[test]
fn other_input_enqueues_nothing() {
    let (tx, rx) = sync_channel(8);
    handle_key_event(0, WM_KEYDOWN, 0x41, &tx);
    handle_key_event(0, 0x0101, VK_VOLUME_UP, &tx);
    handle_key_event(-1, WM_KEYDOWN, VK_VOLUME_UP, &tx);
    assert_eq!(rx.try_iter().count(), 0);
}

#[test]
fn never_blocks_when_full() {
    let (tx, rx) = sync_channel(1);
    for _ in 0..1000 {
        handle_key_event(0, WM_KEYDOWN, VK_VOLUME_UP, &tx);
    }
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn never_blocks_when_receiver_gone() {
    let (tx, rx) = sync_channel(1);
    drop(rx);
    for _ in 0..1000 {
        handle_key_event(0, WM_KEYDOWN, VK_VOLUME_UP, &tx);
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
        handle_key_event(0, WM_KEYDOWN, std::hint::black_box(vk), &tx);
    }
    let mean = started.elapsed() / CALLS;
    println!("mean per call: {mean:?}");
    assert!(mean.as_micros() < 5, "mean {mean:?} per call");
}
