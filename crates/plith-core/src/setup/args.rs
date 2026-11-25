//! Command-line parsing for the installer and its elevated worker.

use super::Version;

/// The pipe name prefix a worker accepts.
const PIPE_PREFIX: &str = "\\\\.\\pipe\\plith-setup-";

/// What the elevated worker should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// Install or repair.
    Install,
    /// Remove.
    Uninstall,
}

/// How this process was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// The normal installer window.
    Install,
    /// Started by the update check: `--update --from <version>`.
    Update {
        /// The version being replaced.
        from: Version,
    },
    /// `--uninstall`.
    Uninstall,
    /// The elevated worker: `--work <install|uninstall> <pipe>`.
    Work {
        /// The job.
        job: Job,
        /// The pipe to report on.
        pipe: String,
    },
}

/// Parses the arguments after the program name; anything unknown gives `Install`.
#[must_use]
pub fn parse_args(args: &[String]) -> Mode {
    let arg = |i: usize| args.get(i).map(String::as_str);
    match arg(0) {
        Some("--uninstall") => Mode::Uninstall,
        Some("--update") if arg(1) == Some("--from") => arg(2)
            .and_then(Version::parse)
            .map_or(Mode::Install, |from| Mode::Update { from }),
        Some("--work") => {
            let job = match arg(1) {
                Some("install") => Job::Install,
                Some("uninstall") => Job::Uninstall,
                _ => return Mode::Install,
            };
            match arg(2) {
                Some(pipe) if pipe.starts_with(PIPE_PREFIX) => Mode::Work {
                    job,
                    pipe: pipe.to_string(),
                },
                _ => Mode::Install,
            }
        }
        _ => Mode::Install,
    }
}

/// Quotes one argument by the Windows command-line rules.
#[must_use]
pub fn quote_arg(arg: &str) -> String {
    let plain = !arg.is_empty()
        && !arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '"' | '(' | ')'));
    if plain {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}
