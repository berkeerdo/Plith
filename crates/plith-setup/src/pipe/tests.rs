//! Tests of the report pipe: server and client in one process.

use super::{ReportServer, report_client, report_pipe_name};
use plith_core::setup::Report;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(5);

fn unique_name() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    plith_core::setup::setup_pipe_name(1_000_000 + n, std::process::id())
}

#[test]
fn pipe_carries_reports_in_order() {
    let name = unique_name();
    let server = ReportServer::bind(&name).expect("bind");
    let client = report_client(&name).expect("client");
    let sent = [
        Report::Step(1),
        Report::Log("copying plith.exe".to_string()),
        Report::Step(2),
        Report::Failed {
            step: 3,
            reason: "5".to_string(),
        },
        Report::Finished,
    ];
    for report in &sent {
        client.send(report).expect("send");
    }
    for expected in &sent {
        assert_eq!(server.next(WAIT).as_ref(), Some(expected));
    }
}

#[test]
fn nothing_arrives_when_nothing_is_sent() {
    let name = unique_name();
    let server = ReportServer::bind(&name).expect("bind");
    assert_eq!(server.next(Duration::from_millis(50)), None);
}

#[test]
fn a_second_server_cannot_take_the_name() {
    let name = unique_name();
    let _first = ReportServer::bind(&name).expect("bind");
    assert!(ReportServer::bind(&name).is_err());
}

#[test]
fn a_missing_pipe_is_an_error_for_the_client() {
    assert!(report_client(&unique_name()).is_err());
}

#[test]
fn dropping_an_unused_server_frees_the_name() {
    let name = unique_name();
    drop(ReportServer::bind(&name).expect("bind"));
    // The helper thread ends asynchronously; the name is free once it has closed the pipe.
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        match ReportServer::bind(&name) {
            Ok(_) => break,
            Err(error) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "name stayed taken: {error}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

#[test]
fn the_window_names_its_pipe_by_session_and_pid() {
    let name = report_pipe_name();
    assert!(name.starts_with("\\\\.\\pipe\\plith-setup-"));
    assert!(name.ends_with(&format!("-{}", std::process::id())));
}

#[test]
fn worker_refuses_non_pipe_handle() {
    let path = std::env::temp_dir().join(format!("plith-setup-nonpipe-{}", std::process::id()));
    std::fs::write(&path, b"keep").unwrap();
    let result = super::open_for_writing(&path.display().to_string());
    assert!(result.is_err());
    drop(result);
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn the_client_refuses_names_that_are_not_setup_pipes() {
    assert!(report_client(r"\\.\pipe\plith-setup-1-2\..\..\x").is_err());
    assert!(report_client(r"C:\Windows\win.ini").is_err());
}
