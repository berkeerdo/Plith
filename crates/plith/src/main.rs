//! The Plith app: composes the engine, the shell and the services.

mod app;
mod demo;
mod idle;
mod log;

use plith_core::launch::Launch;

const USAGE: &str = "usage: plith [--demo <name|script>] [--exit-after-demo] [--idle-check <secs>] \
                     [--clock auto|dcomp|dxgi|timer] [--gpu-priority <-7..7>] \
                     [--sched-class normal|above|high] [--latency <1..3>] [--sync <0|1>] \
                     [--mmcss normal|high|critical] [--tune r1|brisk]";

fn main() {
    match Launch::parse(std::env::args().skip(1)) {
        Ok(launch) => std::process::exit(app::run(&launch)),
        Err(error) => {
            eprintln!("plith: {error}\n{USAGE}");
            std::process::exit(2);
        }
    }
}
