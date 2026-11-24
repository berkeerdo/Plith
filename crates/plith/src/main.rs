//! The Plith app: composes the engine, the shell and the services.
// Release runs without a console window; debug keeps it for eprintln and demo output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod demo;
mod idle;
mod instance;
mod live;
mod locale;
mod log;
mod model;
mod osd;
mod pages;
mod plays_file;
mod services;
mod settings_file;
mod shelf_file;

use plith_core::launch::Launch;

const USAGE: &str = "usage: plith [--demo <name|script>] [--exit-after-demo] [--idle-check <secs>] \
                     [--clock auto|dcomp|dxgi|timer] [--gpu-priority <-7..7>] \
                     [--sched-class normal|above|high] [--latency <1..3>] [--sync <0|1>] \
                     [--mmcss normal|high|critical] [--tune r1|brisk] \
                     [--fixture banana|potato|bare|none] \
                     [--flyout off|dry|hide] \
                     [--idle-look karot|clock|playing|seal|plain|hidden] \
                     [--theme gece|kagit|york|sodyum]";

fn main() {
    match Launch::parse(std::env::args().skip(1)) {
        Ok(launch) => std::process::exit(app::run(&launch)),
        Err(error) => {
            // The release build has no console, so the log file is where a bad flag shows.
            if let Ok(log) = log::LogFile::open() {
                log.line(&format!("launch error: {error}"));
            }
            eprintln!("plith: {error}\n{USAGE}");
            std::process::exit(2);
        }
    }
}
