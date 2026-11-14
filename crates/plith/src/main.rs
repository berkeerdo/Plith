//! The Plith app: composes the engine, the shell and the services.

mod app;
mod demo;
mod idle;
mod log;

use plith_core::launch::Launch;

const USAGE: &str = "usage: plith [--demo <name|script>] [--exit-after-demo] [--idle-check <secs>] \
                     [--clock auto|dcomp|dxgi|timer] [--hit region|split|mask]";

fn main() {
    match Launch::parse(std::env::args().skip(1)) {
        Ok(launch) => std::process::exit(app::run(&launch)),
        Err(error) => {
            eprintln!("plith: {error}\n{USAGE}");
            std::process::exit(2);
        }
    }
}
