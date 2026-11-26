//! The Plith app: composes the engine, the shell and the services.
// Release runs without a console window; debug keeps it for eprintln and demo output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod demo;
mod idle;
mod instance;
mod live;
mod model;
mod osd;
mod pages;
mod plays_file;
mod services;
mod settings_file;
mod shelf_file;
mod update;

use plith_core::launch::Launch;
use plith_core::theme::registry::Registry;

const USAGE: &str = "usage: plith [--demo <name|script>] [--exit-after-demo] [--idle-check <secs>] \
                     [--clock auto|dcomp|dxgi|timer] [--gpu-priority <-7..7>] \
                     [--sched-class normal|above|high] [--latency <1..3>] [--sync <0|1>] \
                     [--mmcss normal|high|critical] [--tune r1|brisk] \
                     [--fixture banana|potato|bare|none] \
                     [--flyout off|dry|hide] \
                     [--idle-look karot|clock|playing|seal|plain|hidden] \
                     [--theme <id>]";

/// The launch error for a `--theme` the registry does not know, in the style of the parser's.
fn unknown_theme(launch: &Launch) -> Option<String> {
    let id = launch.theme.as_ref()?;
    let known = Registry::builtin().get(id).is_some();
    (!known).then(|| format!("unknown theme `{}`", id.as_str()))
}

fn main() {
    let parsed = Launch::parse(std::env::args().skip(1)).map_err(|error| error.to_string());
    let checked = parsed.and_then(|launch| match unknown_theme(&launch) {
        Some(error) => Err(error),
        None => Ok(launch),
    });
    match checked {
        Ok(launch) => std::process::exit(app::run(&launch)),
        Err(error) => {
            // The release build has no console, so the log file is where a bad flag shows.
            if let Ok(log) = plith_host::log::LogFile::open() {
                log.line(&format!("launch error: {error}"));
            }
            eprintln!("plith: {error}\n{USAGE}");
            std::process::exit(2);
        }
    }
}
