//! Sends a demo sequence to an OSD helper. It only sends messages and writes to no hardware.
//!
//! Usage: `cargo run --package plith-ipc --example send_osd -- [--pipe NAME] [--seconds N] [--dev]`
//!
//! `--dev` also sends to a helper at this process's own integrity level (an unsigned dev
//! run); without it only the installed, more trusted helper is accepted.
//!
//! With no server it keeps retrying and exits after `--seconds` (default 10).

#[cfg(windows)]
fn main() {
    use std::time::{Duration, Instant};

    use plith_ipc::{Message, Meter, OsdLink, ServerTrust, current_pipe_name};

    let mut pipe = current_pipe_name();
    let mut seconds = 10u64;
    let mut trust = ServerTrust::Installed;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--dev" {
            trust = ServerTrust::DevSameUser;
            continue;
        }
        match (arg.as_str(), args.next()) {
            ("--pipe", Some(value)) => pipe = value,
            ("--seconds", Some(value)) => seconds = value.parse().unwrap_or(seconds),
            _ => {
                eprintln!("usage: send_osd [--pipe NAME] [--seconds N] [--dev]");
                return;
            }
        }
    }

    let deadline = Instant::now() + Duration::from_secs(seconds);
    let pause = |time: Duration| {
        let left = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(time.min(left));
    };
    let level = |meter, percent, muted: bool, label: &str| Message::Level {
        meter,
        percent,
        muted,
        label: label.to_string(),
    };

    println!("sending to {pipe:?} for at most {seconds} s ({trust:?})");
    let link = OsdLink::start_with(pipe, trust);
    let report = |link: &OsdLink| {
        println!(
            "link: connected={} last_error={:?}",
            link.is_connected(),
            link.last_error()
        );
    };
    pause(Duration::from_millis(300));
    report(&link);
    for percent in (0..=100).step_by(10) {
        let _ = link.send(level(Meter::Volume, percent, false, "Speakers"));
        pause(Duration::from_millis(150));
    }
    let _ = link.send(level(Meter::Volume, 100, true, "Speakers"));
    pause(Duration::from_millis(500));
    let _ = link.send(level(Meter::Brightness, 50, false, "Display"));
    pause(Duration::from_millis(500));
    let _ = link.send(Message::Hide);
    pause(Duration::from_millis(300));
    report(&link);
    drop(link);
    println!("done");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("send_osd runs on Windows only");
}
