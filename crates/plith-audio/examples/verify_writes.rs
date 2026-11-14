//! Hardware write verification for plith-audio. THIS EXAMPLE CHANGES THE MACHINE.
//!
//! Approval requirement: run it with `--yes` only after the owner has approved it. Without
//! `--yes` it reads the current values, prints exactly what it WOULD change and how it
//! restores it, then exits 0 having written nothing.
//!
//! Sections (none selected with `--yes` does nothing; they always run in this order):
//! `--endpoint-volume`  volume of the attached endpoint +0.02, then back to the original.
//! `--endpoint-mute`    mute toggled twice, ending in the original state.
//! `--mic-mute`         default capture device mute toggled twice.
//! `--default-endpoint --to <id substring>` default render device switched to the one
//!                      endpoint whose id contains the text, then back (Console and
//!                      Multimedia roles, re-read after each step).
//! `--voicemeeter-gain` strip 0 gain +0.5 dB, then back (skipped when Voicemeeter is not running).
//!
//! Every change sits behind a drop guard, so the original value is restored when a step fails
//! or panics. Each section prints BEFORE, AFTER-CHANGE and AFTER-RESTORE and PASS or FAIL.

#[cfg(windows)]
#[path = "verify_writes/devices.rs"]
mod devices;
#[cfg(windows)]
#[path = "verify_writes/endpoint.rs"]
mod endpoint;
#[cfg(windows)]
#[path = "verify_writes/support.rs"]
mod support;

#[cfg(windows)]
mod cli {
    use plith_audio::endpoints::{EndpointService, Status};
    use plith_audio::microphone::MicrophoneService;
    use plith_audio::voicemeeter::VoicemeeterService;

    use crate::devices::{mic_mute, voicemeeter_gain};
    use crate::endpoint::{default_endpoint, endpoint_mute, endpoint_volume};
    use crate::support::{DO_NOT_INTERRUPT, Verdict, install_ctrl_handler, interrupted};

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Section {
        EndpointVolume,
        EndpointMute,
        MicMute,
        DefaultEndpoint,
        VoicemeeterGain,
    }

    struct Args {
        yes: bool,
        sections: Vec<Section>,
        to: String,
    }

    impl Args {
        fn has(&self, section: Section) -> bool {
            self.sections.contains(&section)
        }
    }

    fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
        let mut out = Args {
            yes: false,
            sections: Vec::new(),
            to: String::new(),
        };
        let mut args = args.skip(1);
        while let Some(arg) = args.next() {
            let section = match arg.as_str() {
                "--yes" => {
                    out.yes = true;
                    continue;
                }
                "--to" => {
                    out.to = args.next().ok_or("--to needs a value")?;
                    continue;
                }
                "--endpoint-volume" => Section::EndpointVolume,
                "--endpoint-mute" => Section::EndpointMute,
                "--mic-mute" => Section::MicMute,
                "--default-endpoint" => Section::DefaultEndpoint,
                "--voicemeeter-gain" => Section::VoicemeeterGain,
                other => return Err(format!("unknown argument {other}")),
            };
            out.sections.push(section);
        }
        if out.has(Section::DefaultEndpoint) && out.to.is_empty() {
            return Err("--default-endpoint needs --to <endpoint id substring>".into());
        }
        Ok(out)
    }

    fn start_endpoints() -> Result<EndpointService, String> {
        let service = EndpointService::new();
        match service.start() {
            Status::Running => Ok(service),
            Status::Unavailable(reason) => Err(reason),
        }
    }

    fn start_mic() -> Result<MicrophoneService, String> {
        let service = MicrophoneService::new();
        match service.start() {
            Status::Running => Ok(service),
            Status::Unavailable(reason) => Err(reason),
        }
    }

    type Results = Vec<(&'static str, Verdict)>;

    fn endpoint_sections(args: &Args, results: &mut Results) {
        let wanted = [
            (Section::EndpointVolume, "endpoint-volume"),
            (Section::EndpointMute, "endpoint-mute"),
            (Section::DefaultEndpoint, "default-endpoint"),
        ];
        if !wanted.iter().any(|(section, _)| args.has(*section)) {
            return;
        }
        let service = match start_endpoints() {
            Ok(service) => service,
            Err(reason) => {
                for (section, name) in wanted.iter().filter(|(s, _)| args.has(*s)) {
                    let _ = section;
                    results.push((name, Verdict::Skipped(format!("unavailable: {reason}"))));
                }
                return;
            }
        };
        if args.has(Section::EndpointVolume) {
            println!("\n== endpoint volume ==");
            results.push(("endpoint-volume", endpoint_volume(&service, args.yes)));
        }
        if args.has(Section::EndpointMute) {
            println!("\n== endpoint mute ==");
            results.push(("endpoint-mute", endpoint_mute(&service, args.yes)));
        }
        if args.has(Section::DefaultEndpoint) {
            println!("\n== default endpoint ==");
            let verdict = default_endpoint(&service, &args.to, args.yes);
            results.push(("default-endpoint", verdict));
        }
    }

    fn summarize(results: &Results) -> bool {
        println!("\n== summary ==");
        let mut ok = true;
        for (name, verdict) in results {
            match verdict {
                Verdict::Pass => println!("{name}: PASS"),
                Verdict::Fail(why) => {
                    ok = false;
                    println!("{name}: FAIL ({why})");
                }
                Verdict::Skipped(why) => println!("{name}: SKIPPED ({why})"),
                Verdict::Planned => println!("{name}: planned only"),
            }
        }
        ok
    }

    /// Runs the selected sections; `false` when any failed.
    pub fn run() -> bool {
        let args = match parse(std::env::args()) {
            Ok(args) => args,
            Err(why) => {
                eprintln!("{why}");
                std::process::exit(2);
            }
        };
        if args.sections.is_empty() {
            println!("no section selected, nothing to do");
            return true;
        }
        println!(
            "{}",
            if args.yes {
                "WRITE MODE (--yes): values below are changed and restored"
            } else {
                "DRY PLAN (no --yes): nothing is written"
            }
        );
        if args.yes {
            println!("{DO_NOT_INTERRUPT}");
            install_ctrl_handler();
        }
        let mut results = Results::new();
        endpoint_sections(&args, &mut results);
        if args.has(Section::MicMute) && !interrupted() {
            println!("\n== mic mute ==");
            let verdict = start_mic().map_or_else(Verdict::Skipped, |s| mic_mute(&s, args.yes));
            results.push(("mic-mute", verdict));
        }
        if args.has(Section::VoicemeeterGain) && !interrupted() {
            println!("\n== voicemeeter gain ==");
            let verdict = voicemeeter_gain(&VoicemeeterService::new(), args.yes);
            results.push(("voicemeeter-gain", verdict));
        }
        summarize(&results)
    }
}

#[cfg(windows)]
fn main() {
    if !cli::run() {
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    println!("Core Audio is Windows only");
}
