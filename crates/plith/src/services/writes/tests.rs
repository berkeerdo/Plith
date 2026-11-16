use super::*;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(5);

#[test]
fn writes_run_in_order_on_the_writes_thread() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let (done_tx, done_rx) = mpsc::channel();
    let mut writer = Writer::spawn(move |write| {
        let name = std::thread::current().name().map(str::to_string);
        sink.lock().unwrap().push((write, name));
        let _ = done_tx.send(());
    })
    .expect("spawned");
    writer.send(Write::ToggleMic);
    writer.send(Write::Route { strip: 1, bus: 2 });
    writer.send(Write::Seek(Duration::from_secs(3)));
    for _ in 0..3 {
        done_rx.recv_timeout(BOUND).expect("applied");
    }
    writer.stop();
    let thread = Some(String::from("plith-writes"));
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            (Write::ToggleMic, thread.clone()),
            (Write::Route { strip: 1, bus: 2 }, thread.clone()),
            (Write::Seek(Duration::from_secs(3)), thread),
        ]
    );
}

#[test]
fn a_slow_write_does_not_block_the_sender() {
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    let mut writer = Writer::spawn(move |write| {
        if write == Write::PlayPause {
            let _ = release_rx.recv_timeout(BOUND);
        }
        let _ = done_tx.send(write);
    })
    .expect("spawned");
    writer.send(Write::PlayPause);
    writer.send(Write::Next);
    assert!(
        done_rx.try_recv().is_err(),
        "both sends returned while the first write still runs"
    );
    release_tx.send(()).unwrap();
    assert_eq!(done_rx.recv_timeout(BOUND), Ok(Write::PlayPause));
    assert_eq!(done_rx.recv_timeout(BOUND), Ok(Write::Next));
    writer.stop();
    writer.stop();
    writer.send(Write::Previous);
    assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
}
