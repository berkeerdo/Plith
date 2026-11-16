use super::Slot;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn newer_value_replaces_older() {
    let slot = Slot::new();
    slot.put(1);
    slot.put(2);
    assert_eq!(slot.take_wait(Duration::from_millis(10)), Some(2));
    assert_eq!(slot.take_wait(Duration::from_millis(10)), None);
}

#[test]
fn take_wait_times_out_empty() {
    let slot: Slot<u8> = Slot::new();
    let start = Instant::now();
    assert_eq!(slot.take_wait(Duration::from_millis(50)), None);
    assert!(start.elapsed() >= Duration::from_millis(40));
}

#[test]
fn close_wakes_a_waiter() {
    let slot: Arc<Slot<u8>> = Arc::new(Slot::new());
    let waiter = {
        let slot = Arc::clone(&slot);
        thread::spawn(move || slot.take_wait(Duration::from_secs(5)))
    };
    thread::sleep(Duration::from_millis(50));
    let start = Instant::now();
    slot.close();
    assert_eq!(waiter.join().unwrap(), None);
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(slot.is_closed());
}

#[test]
fn pending_value_survives_close_once() {
    let slot = Slot::new();
    slot.put(7);
    slot.close();
    assert_eq!(slot.take_wait(Duration::from_secs(1)), Some(7));
    assert_eq!(slot.take_wait(Duration::from_secs(1)), None);
}

#[test]
fn put_wakes_a_waiter() {
    let slot: Arc<Slot<u8>> = Arc::new(Slot::new());
    let waiter = {
        let slot = Arc::clone(&slot);
        thread::spawn(move || slot.take_wait(Duration::from_secs(5)))
    };
    thread::sleep(Duration::from_millis(50));
    slot.put(9);
    assert_eq!(waiter.join().unwrap(), Some(9));
}
