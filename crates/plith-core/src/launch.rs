//! Command-line launch options.

use crate::demo::{DemoError, DemoScript};
use std::fmt;

/// Which frame clock the render thread should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockRequest {
    /// Pick the best available clock.
    #[default]
    Auto,
    /// The DirectComposition frame statistics clock.
    Dcomp,
    /// The DXGI vblank clock.
    Dxgi,
    /// A plain timer.
    Timer,
}

/// Which hit-testing candidate the shell uses (a spike, see the spikes doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HitMode {
    /// `SetWindowRgn` on the single window.
    #[default]
    Region,
    /// A second, input-only window under the visual one.
    Split,
    /// A layered near-transparent alpha mask window.
    Mask,
}

/// Parsed launch options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Launch {
    /// Demo script to run instead of waiting for input.
    pub demo: Option<DemoScript>,
    /// Seconds of idle time after which to check for leaks, if requested.
    pub idle_check_secs: Option<u64>,
    /// Requested frame clock.
    pub clock: ClockRequest,
    /// Exit once the demo has finished.
    pub exit_after_demo: bool,
    /// Hit-testing candidate.
    pub hit: HitMode,
}

/// Why the arguments were rejected; each variant carries the offending text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The flag is not known.
    UnknownFlag(String),
    /// The flag needs a value and none followed.
    MissingValue(String),
    /// The flag's value is not valid.
    BadValue {
        /// The flag.
        flag: String,
        /// The rejected value.
        value: String,
    },
    /// The `--demo` script is invalid.
    Demo(DemoError),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFlag(flag) => write!(f, "unknown flag: {flag}"),
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::BadValue { flag, value } => write!(f, "bad value for {flag}: {value}"),
            Self::Demo(error) => write!(f, "invalid demo script: {error}"),
        }
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Demo(error) => Some(error),
            _ => None,
        }
    }
}

fn parse_clock(value: &str) -> Option<ClockRequest> {
    match value {
        "auto" => Some(ClockRequest::Auto),
        "dcomp" => Some(ClockRequest::Dcomp),
        "dxgi" => Some(ClockRequest::Dxgi),
        "timer" => Some(ClockRequest::Timer),
        _ => None,
    }
}

fn parse_hit(value: &str) -> Option<HitMode> {
    match value {
        "region" => Some(HitMode::Region),
        "split" => Some(HitMode::Split),
        "mask" => Some(HitMode::Mask),
        _ => None,
    }
}

impl Launch {
    /// Parses the arguments, without the program name.
    ///
    /// # Errors
    /// Returns a [`LaunchError`] for an unknown flag, a missing or bad value or a bad script.
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self, LaunchError> {
        let mut launch = Self::default();
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| LaunchError::MissingValue(flag.clone()))
            };
            let bad = |value: String| LaunchError::BadValue {
                flag: flag.clone(),
                value,
            };
            match flag.as_str() {
                "--demo" => {
                    let text = value()?;
                    launch.demo = Some(match DemoScript::builtin(&text) {
                        Some(script) => script,
                        None => DemoScript::parse(&text).map_err(LaunchError::Demo)?,
                    });
                }
                "--idle-check" => {
                    let text = value()?;
                    launch.idle_check_secs = Some(text.parse().map_err(|_| bad(text))?);
                }
                "--clock" => {
                    let text = value()?;
                    launch.clock = parse_clock(&text).ok_or_else(|| bad(text))?;
                }
                "--hit" => {
                    let text = value()?;
                    launch.hit = parse_hit(&text).ok_or_else(|| bad(text))?;
                }
                "--exit-after-demo" => launch.exit_after_demo = true,
                _ => return Err(LaunchError::UnknownFlag(flag)),
            }
        }
        Ok(launch)
    }
}

#[cfg(test)]
mod tests;
