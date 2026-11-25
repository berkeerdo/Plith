//! `plith-setup.exe`: the installer. Composes only; the steps live in their modules.

#![windows_subsystem = "windows"]

mod app;
mod art;
mod elevate;
#[allow(dead_code, reason = "the window uses only part of the install steps")]
mod files;
mod payload;
mod pipe;
mod processes;
mod register;
#[cfg(test)]
mod testing;
#[allow(dead_code, reason = "the window uses only part of the uninstall steps")]
mod uninstall;
mod worker;

use plith_core::setup::{Mode, parse_args};
use windows::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};

fn main() {
    // First, before anything can load a DLL: System32 only (the linker flag covers the DLLs
    // loaded at start-up). Nothing to do if it fails.
    // SAFETY: a plain call with a flag constant.
    let _ = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Mode::Work { job, pipe } => std::process::exit(worker::run(job, &pipe)),
        mode => std::process::exit(app::run(&mode)),
    }
}
