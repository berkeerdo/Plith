//! Tests of the worker's refusal and its error-code extraction. Nothing here installs.

use super::{os_code, run};
use crate::pipe::ReportServer;
use crate::testing::TempDir;
use plith_core::setup::Job;
use plith_ipc::security::own_integrity_above_medium;
use std::time::Duration;

#[test]
fn worker_refuses_at_medium() {
    // A test run from an elevated shell would be a real install: never run it there.
    if own_integrity_above_medium() != Ok(false) {
        return;
    }
    let temp = TempDir::new("worker-refuse");
    let marker = temp.root().join("marker.txt");
    std::fs::create_dir_all(temp.root()).unwrap();
    std::fs::write(&marker, b"untouched").unwrap();
    let pipe = plith_core::setup::setup_pipe_name(2_000_000, std::process::id());
    let server = ReportServer::bind(&pipe).expect("bind");

    assert_eq!(run(Job::Install, &pipe), 2);
    assert_eq!(run(Job::Uninstall, &pipe), 2);

    assert_eq!(server.next(Duration::from_millis(300)), None);
    assert_eq!(std::fs::read(&marker).unwrap(), b"untouched");
}

#[test]
fn os_error_codes_come_out_in_decimal() {
    assert_eq!(
        os_code("cannot create C:\\x: Access is denied. (os error 5)"),
        5
    );
    assert_eq!(
        os_code("cannot trust the certificate: Access is denied. (0x80070005)"),
        5
    );
    assert_eq!(os_code("x (0x8007045A)"), 0x45A);
    assert_eq!(os_code("x (0x80004005)"), 0x8000_4005);
}

#[test]
fn text_without_a_code_gives_zero() {
    assert_eq!(os_code("C:\\Program Files\\Plith is a link"), 0);
    assert_eq!(os_code("(os error banana) and (0xZZ)"), 0);
    assert_eq!(os_code(""), 0);
}

#[test]
fn the_last_code_wins() {
    assert_eq!(os_code("a (os error 2) then b (os error 32)"), 32);
}
