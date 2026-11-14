//! The UIAccess helper that draws the volume, brightness and mute OSD over exclusive fullscreen games.
//!
//! Exit codes: 0 done, 1 the spike failed (the log says why, for example a band call that
//! Windows refused without UIAccess), 2 bad arguments.

mod args;
mod band;
mod log;
mod spike;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    let log = Arc::new(log::Log::open());
    let words: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match args::parse(&words) {
        Ok(parsed) => parsed,
        Err(error) => {
            log.line(&format!("args: {error}"));
            eprintln!("plith-osd: {error}");
            return ExitCode::from(2);
        }
    };
    let Some(creation) = parsed.spike else {
        log.line("start: no --spike given, nothing to do yet");
        return ExitCode::SUCCESS;
    };
    log.line(&format!(
        "start: spike {} for {} s, pipe {}",
        creation.as_str(),
        parsed.seconds,
        parsed.pipe.as_deref().unwrap_or("none")
    ));
    match spike::run(creation, parsed.seconds, &log) {
        Ok(()) => {
            log.line(&format!("spike {} done", creation.as_str()));
            ExitCode::SUCCESS
        }
        Err(error) => {
            log.line(&format!("spike {} failed: {error}", creation.as_str()));
            ExitCode::FAILURE
        }
    }
}
