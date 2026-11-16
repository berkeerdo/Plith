//! The UIAccess helper that draws the volume, brightness and mute OSD over exclusive fullscreen games.
//!
//! Without `--spike` it runs the OSD on `--pipe NAME` (default: this session's pipe). With
//! `--spike` it runs the Task 2 band spike instead.
//!
//! Exit codes: 0 done (also when another helper already serves the pipe), 1 failed (the log
//! says why), 2 bad arguments.

mod app;
mod args;
mod band;
mod log;
mod model;
mod paint;
mod render;
mod spike;
mod watch;

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
    if let Some(creation) = parsed.spike {
        return run_spike(creation, parsed.seconds, parsed.pipe.as_deref(), &log);
    }
    let pipe = parsed.pipe.unwrap_or_else(plith_ipc::current_pipe_name);
    run_app(&pipe, &log)
}

fn run_app(pipe: &str, log: &Arc<log::Log>) -> ExitCode {
    match app::run(pipe, log) {
        Ok(()) => {
            log.line("osd done");
            ExitCode::SUCCESS
        }
        Err(error) => {
            log.line(&format!("osd failed: {error}"));
            ExitCode::FAILURE
        }
    }
}

fn run_spike(
    creation: band::Creation,
    seconds: u32,
    pipe: Option<&str>,
    log: &Arc<log::Log>,
) -> ExitCode {
    log.line(&format!(
        "start: spike {} for {} s, pipe {:?}",
        creation.as_str(),
        seconds,
        pipe.unwrap_or("none")
    ));
    match spike::run(creation, seconds, log) {
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
