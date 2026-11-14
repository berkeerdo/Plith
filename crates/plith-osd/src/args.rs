//! Command line: `--spike [inband|setband|plain]`, `--seconds N`, `--pipe NAME`.

use crate::band::Creation;

/// Default spike length in seconds.
const DEFAULT_SECONDS: u32 = 20;
/// Longest spike in seconds.
const MAX_SECONDS: u32 = 120;

/// Parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// The spike to run, `None` when `--spike` is absent.
    pub spike: Option<Creation>,
    /// How long the spike runs, 1 to 120 seconds.
    pub seconds: u32,
    /// The pipe name for the IPC channel.
    pub pipe: Option<String>,
}

fn creation(word: &str) -> Option<Creation> {
    match word {
        "inband" => Some(Creation::InBand),
        "setband" => Some(Creation::SetBand),
        "plain" => Some(Creation::Plain),
        _ => None,
    }
}

fn seconds(value: Option<&String>) -> Result<u32, String> {
    let value = value.ok_or("--seconds needs a value")?;
    match value.parse::<u32>() {
        Ok(n @ 1..=MAX_SECONDS) => Ok(n),
        _ => Err(format!("--seconds must be 1 to {MAX_SECONDS}, got {value}")),
    }
}

/// Parses the arguments after the program name. `--spike` takes an optional mode word and
/// defaults to `inband`; `--seconds` defaults to 20.
///
/// # Errors
/// Returns a message naming the bad argument.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        spike: None,
        seconds: DEFAULT_SECONDS,
        pipe: None,
    };
    let mut rest = args.iter().peekable();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--spike" => {
                let mode = rest.peek().and_then(|word| creation(word));
                if mode.is_some() {
                    rest.next();
                }
                parsed.spike = Some(mode.unwrap_or(Creation::InBand));
            }
            "--seconds" => parsed.seconds = seconds(rest.next())?,
            "--pipe" => {
                let name = rest.next().ok_or("--pipe needs a name")?;
                parsed.pipe = Some(name.clone());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests;
