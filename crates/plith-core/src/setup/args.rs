//! Command-line parsing for the installer and its elevated worker.

use super::Version;

/// The pipe name prefix a worker accepts.
const PIPE_PREFIX: &str = "\\\\.\\pipe\\plith-setup-";

/// The pipe name of a setup window: the prefix, the session id and the window's process id.
#[must_use]
pub fn setup_pipe_name(session: u32, pid: u32) -> String {
    format!("{PIPE_PREFIX}{session}-{pid}")
}

/// Reads `(session, pid)` from a setup pipe name. Only `<digits>-<digits>` after the prefix is
/// accepted, so nothing else (a path piece, a second dash, a sign) can ride along.
#[must_use]
pub fn parse_pipe_name(name: &str) -> Option<(u32, u32)> {
    let rest = name.strip_prefix(PIPE_PREFIX)?;
    let (session, pid) = rest.split_once('-')?;
    let number = |text: &str| {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        text.parse::<u32>().ok()
    };
    Some((number(session)?, number(pid)?))
}

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
            // The name is rebuilt from the two numbers, never taken as given.
            match arg(2).and_then(parse_pipe_name) {
                Some((session, pid)) => Mode::Work {
                    job,
                    pipe: setup_pipe_name(session, pid),
                },
                None => Mode::Install,
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
