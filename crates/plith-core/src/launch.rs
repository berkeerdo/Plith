//! Command-line launch options.

use crate::demo::{DemoError, DemoScript};
use crate::levers::{Levers, MmcssPriority, SchedClass};
use crate::settings::{FlyoutPref, IdleLook};
use crate::tuning::TuningSet;
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

/// Mock data that replaces the hardware services (`--fixture`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// Voicemeeter Banana as the mock draws it.
    Banana,
    /// Voicemeeter Potato: eight strips, eight buses.
    Potato,
    /// No Voicemeeter: Windows endpoints only.
    Bare,
}

/// What to do with the Windows volume and brightness flyout (`--flyout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlyoutMode {
    /// Leave the flyout alone.
    #[default]
    Off,
    /// Find the flyout and log what would be hidden, without hiding it.
    DryRun,
    /// Hide the flyout while the OSD is connected.
    Hide,
}

/// The flyout mode of a run: `--flyout` when given, else the settings file's choice.
#[must_use]
pub fn flyout_mode(cli: Option<FlyoutMode>, pref: FlyoutPref) -> FlyoutMode {
    cli.unwrap_or(match pref {
        FlyoutPref::Off => FlyoutMode::Off,
        FlyoutPref::Hide => FlyoutMode::Hide,
    })
}

/// Parsed launch options.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Launch {
    /// Demo script to run instead of waiting for input.
    pub demo: Option<DemoScript>,
    /// Seconds of idle time after which to check for leaks, if requested.
    pub idle_check_secs: Option<u64>,
    /// Requested frame clock.
    pub clock: ClockRequest,
    /// Exit once the demo has finished.
    pub exit_after_demo: bool,
    /// Load levers; the default is the measured R1b pick.
    pub levers: Levers,
    /// Spring tuning preset; the default is BRISK (R1 stays as `--tune r1`).
    pub tune: TuningSet,
    /// Mock data instead of the real services; `None` runs the real services.
    pub fixture: Option<Fixture>,
    /// What to do with the Windows volume and brightness flyout.
    pub flyout: Option<FlyoutMode>,
    /// The resting look for this run, replacing the settings file's (`--idle-look <key>`).
    pub idle_look: Option<IdleLook>,
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

fn parse_fixture(value: &str) -> Option<Fixture> {
    match value {
        "banana" => Some(Fixture::Banana),
        "potato" => Some(Fixture::Potato),
        "bare" => Some(Fixture::Bare),
        _ => None,
    }
}

fn parse_flyout(value: &str) -> Option<FlyoutMode> {
    match value {
        "off" => Some(FlyoutMode::Off),
        "dry" => Some(FlyoutMode::DryRun),
        "hide" => Some(FlyoutMode::Hide),
        _ => None,
    }
}

fn parse_sched(value: &str) -> Option<SchedClass> {
    match value {
        "normal" => Some(SchedClass::Normal),
        "above" => Some(SchedClass::AboveNormal),
        "high" => Some(SchedClass::High),
        _ => None,
    }
}

fn parse_mmcss(value: &str) -> Option<MmcssPriority> {
    match value {
        "normal" => Some(MmcssPriority::Normal),
        "high" => Some(MmcssPriority::High),
        "critical" => Some(MmcssPriority::Critical),
        _ => None,
    }
}

/// Parses `text` as an integer inside `range`.
fn parse_in<T: std::str::FromStr + PartialOrd>(
    text: &str,
    range: std::ops::RangeInclusive<T>,
) -> Option<T> {
    text.parse().ok().filter(|n| range.contains(n))
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
                    // The watchdog grows with the window; 600 s only bounds a typo.
                    launch.idle_check_secs =
                        Some(parse_in(&text, 0..=600).ok_or_else(|| bad(text))?);
                }
                "--clock" => {
                    let text = value()?;
                    launch.clock = parse_clock(&text).ok_or_else(|| bad(text))?;
                }
                "--gpu-priority" => {
                    let text = value()?;
                    launch.levers.gpu_priority =
                        parse_in(&text, -7..=7).ok_or_else(|| bad(text))?;
                }
                "--sched-class" => {
                    let text = value()?;
                    launch.levers.sched_class = parse_sched(&text).ok_or_else(|| bad(text))?;
                }
                "--latency" => {
                    let text = value()?;
                    launch.levers.latency = parse_in(&text, 1..=3).ok_or_else(|| bad(text))?;
                }
                "--sync" => {
                    let text = value()?;
                    launch.levers.sync = parse_in(&text, 0..=1).ok_or_else(|| bad(text))?;
                }
                "--mmcss" => {
                    let text = value()?;
                    launch.levers.mmcss = parse_mmcss(&text).ok_or_else(|| bad(text))?;
                }
                "--tune" => {
                    let text = value()?;
                    launch.tune = TuningSet::by_name(&text).ok_or_else(|| bad(text))?;
                }
                "--fixture" => {
                    let text = value()?;
                    // `none` means the real services.
                    launch.fixture = if text == "none" {
                        None
                    } else {
                        Some(parse_fixture(&text).ok_or_else(|| bad(text))?)
                    };
                }
                "--flyout" => {
                    let text = value()?;
                    launch.flyout = Some(parse_flyout(&text).ok_or_else(|| bad(text))?);
                }
                "--idle-look" => {
                    let text = value()?;
                    launch.idle_look = Some(IdleLook::from_key(&text).ok_or_else(|| bad(text))?);
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
