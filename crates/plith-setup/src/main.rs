//! `plith-setup.exe`: the installer. Composes only; the steps live in their modules.

#![windows_subsystem = "windows"]

// The window (Task 10) uses what the worker does not; until then those parts are dead code.
#[allow(dead_code, reason = "used by the window added in Task 10")]
mod art;
#[allow(dead_code, reason = "used by the window added in Task 10")]
mod elevate;
#[allow(dead_code, reason = "parts are used by the window added in Task 10")]
mod files;
#[allow(dead_code, reason = "parts are used by the window added in Task 10")]
mod payload;
#[allow(dead_code, reason = "used by the window added in Task 10")]
mod pipe;
mod processes;
#[allow(dead_code, reason = "parts are used by the window added in Task 10")]
mod register;
#[cfg(test)]
mod testing;
#[allow(dead_code, reason = "parts are used by the window added in Task 10")]
mod uninstall;
mod worker;

use plith_core::setup::{Mode, parse_args};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Mode::Work { job, pipe } => std::process::exit(worker::run(job, &pipe)),
        // The window arrives in Task 10.
        Mode::Install | Mode::Update { .. } | Mode::Uninstall => {}
    }
}
