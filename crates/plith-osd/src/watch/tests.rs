use super::client;
use crate::log::Log;
use crate::render::Event;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

#[test]
fn dropping_the_watcher_of_a_live_process_returns_at_once_and_sends_nothing() {
    let (events, inbox) = sync_channel(4);
    let watcher = client(std::process::id(), events, Arc::new(Log::closed())).expect("watch");
    std::thread::sleep(Duration::from_millis(50));
    let start = Instant::now();
    drop(watcher);
    assert!(start.elapsed() < Duration::from_millis(500));
    assert!(inbox.try_recv().is_err(), "a cancelled watcher is silent");
}

#[test]
fn an_ended_client_is_reported_once() {
    let mut child = std::process::Command::new(r"C:\Windows\System32\cmd.exe")
        .args(["/d", "/c", "exit 0"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn cmd");
    let pid = child.id();
    let (events, inbox) = sync_channel(4);
    let watcher = client(pid, events, Arc::new(Log::closed())).expect("watch");
    child.wait().expect("wait");
    match inbox.recv_timeout(Duration::from_secs(2)) {
        Ok(Event::ClientGone(gone)) => assert_eq!(gone, pid),
        other => panic!("expected ClientGone, got {other:?}"),
    }
    drop(watcher);
    assert!(inbox.try_recv().is_err(), "reported once");
}
