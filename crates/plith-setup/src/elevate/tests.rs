//! Tests of the elevation mapping. Nothing here starts a process.

use super::{ElevateError, classify, worker_parameters};
use plith_core::setup::{Job, Mode, parse_args};

#[test]
fn cancelled_maps_from_1223() {
    // HRESULT_FROM_WIN32(1223), what the shell reports for a declined prompt.
    assert_eq!(
        classify(0x8007_04C7_u32.cast_signed(), "x"),
        ElevateError::Cancelled
    );
}

#[test]
fn other_errors_keep_their_text() {
    // HRESULT_FROM_WIN32(5), access denied.
    let error = classify(0x8007_0005_u32.cast_signed(), "Access is denied.");
    assert_eq!(
        error,
        ElevateError::Os("ShellExecuteExW: Access is denied.".to_string())
    );
    assert!(matches!(
        classify(0x8000_4005_u32.cast_signed(), "E_FAIL"),
        ElevateError::Os(_)
    ));
}

#[test]
fn parameters_survive_the_argument_parser() {
    let pipe = "\\\\.\\pipe\\plith-setup-1-42";
    let text = worker_parameters(Job::Uninstall, pipe);
    assert_eq!(text, "--work uninstall \\\\.\\pipe\\plith-setup-1-42");
    let args: Vec<String> = text.split(' ').map(str::to_string).collect();
    assert_eq!(
        parse_args(&args),
        Mode::Work {
            job: Job::Uninstall,
            pipe: pipe.to_string()
        }
    );
}
