//! `plith-setup.exe`: the installer. Composes only; the steps live in their modules.

#![windows_subsystem = "windows"]

// Later tasks call these (the worker in Task 9, the window in Task 10); until then only the
// file step's tests do.
#[allow(dead_code, reason = "used by the window added in Task 10")]
mod art;
#[allow(dead_code, reason = "used by the worker added in Task 9")]
mod files;
#[allow(dead_code, reason = "used by the worker added in Task 9")]
mod payload;
#[allow(dead_code, reason = "used by the worker added in Task 9")]
mod processes;

use plith_core::setup::{Mode, parse_args};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Every mode ends here until the elevated worker and the window exist.
    match parse_args(&args) {
        Mode::Install | Mode::Update { .. } | Mode::Uninstall | Mode::Work { .. } => {}
    }
}
