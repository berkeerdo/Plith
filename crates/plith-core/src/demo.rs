//! Demo scripts: a tiny step language that drives the island without any input.

use crate::shapes::PAGES;
use std::fmt;

/// One step of a demo script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoStep {
    /// Expand the island.
    Expand,
    /// Collapse the island.
    Collapse,
    /// Switch to the page with this index.
    Page(usize),
    /// Ask the fixture services to switch to this content variant (0 to 3); ignored without one.
    Content(u8),
    /// Wait this many milliseconds.
    Wait(u64),
}

/// A sequence of steps and how many times to run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoScript {
    /// The steps of one pass.
    pub steps: Vec<DemoStep>,
    /// Number of passes, at least 1.
    pub repeat: u32,
}

/// Why a demo script was rejected; each variant carries the offending text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemoError {
    /// The step is not part of the grammar.
    UnknownStep(String),
    /// A number is missing, malformed or out of its allowed range.
    BadNumber(String),
    /// A `page` step names a page that does not exist.
    PageOutOfRange(String),
    /// `repeat` appears before the last step.
    RepeatNotLast(String),
}

impl fmt::Display for DemoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownStep(s) => write!(f, "unknown demo step: {s}"),
            Self::BadNumber(s) => write!(f, "bad number in demo step: {s}"),
            Self::PageOutOfRange(s) => write!(f, "page does not exist: {s}"),
            Self::RepeatNotLast(s) => write!(f, "repeat must be the last step: {s}"),
        }
    }
}

impl std::error::Error for DemoError {}

const BASIC: &str = "expand; wait 1500; page 1; wait 1200; page 2; wait 1200; page 0; wait 1200; collapse; wait 1500; repeat 3";
const PAGES_DEMO: &str = "content 0; expand; wait 1500; page 1; wait 1200; page 2; wait 1200; page 3; wait 1200; page 0; wait 600; content 1; wait 900; page 1; wait 200; content 2; wait 1300; collapse; wait 1500; repeat 3";
/// The highest content variant a `content` step may name.
const MAX_CONTENT: u64 = 3;
const INTERRUPT: &str = "expand; wait 200; collapse; wait 150; expand; wait 1500; page 1; wait 120; page 0; wait 1500; collapse; wait 100; expand; wait 1500; collapse; wait 1500; repeat 3";

/// A parsed step line: either a real step or the repeat count.
enum Parsed {
    Step(DemoStep),
    Repeat(u32),
}

fn parse_step(text: &str) -> Result<Parsed, DemoError> {
    let mut words = text.split_whitespace();
    let name = words.next().unwrap_or_default();
    let arg = words.next();
    let extra = words.next().is_some();
    let number = |arg: Option<&str>| -> Result<u64, DemoError> {
        match arg {
            Some(a) if !extra => a
                .parse()
                .map_err(|_| DemoError::BadNumber(text.to_string())),
            _ => Err(DemoError::BadNumber(text.to_string())),
        }
    };
    match name {
        "expand" | "collapse" if arg.is_some() => Err(DemoError::UnknownStep(text.to_string())),
        "expand" => Ok(Parsed::Step(DemoStep::Expand)),
        "collapse" => Ok(Parsed::Step(DemoStep::Collapse)),
        "page" => {
            let index = usize::try_from(number(arg)?)
                .map_err(|_| DemoError::PageOutOfRange(text.to_string()))?;
            if index < PAGES.len() {
                Ok(Parsed::Step(DemoStep::Page(index)))
            } else {
                Err(DemoError::PageOutOfRange(text.to_string()))
            }
        }
        "content" => match number(arg)? {
            n if n <= MAX_CONTENT => Ok(Parsed::Step(DemoStep::Content(
                u8::try_from(n).map_err(|_| DemoError::BadNumber(text.to_string()))?,
            ))),
            _ => Err(DemoError::BadNumber(text.to_string())),
        },
        "wait" => Ok(Parsed::Step(DemoStep::Wait(number(arg)?))),
        "repeat" => match u32::try_from(number(arg)?) {
            Ok(n) if n >= 1 => Ok(Parsed::Repeat(n)),
            _ => Err(DemoError::BadNumber(text.to_string())),
        },
        _ => Err(DemoError::UnknownStep(text.to_string())),
    }
}

impl DemoScript {
    /// Parses `step; step; ...`. Whitespace is trimmed, empty steps are ignored and `repeat N`
    /// may only be the last step (default 1).
    ///
    /// # Errors
    /// Returns a [`DemoError`] naming the first offending step.
    pub fn parse(text: &str) -> Result<Self, DemoError> {
        let mut steps = Vec::new();
        let mut repeat = None;
        for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            if repeat.is_some() {
                return Err(DemoError::RepeatNotLast(part.to_string()));
            }
            match parse_step(part)? {
                Parsed::Step(step) => steps.push(step),
                Parsed::Repeat(n) => repeat = Some(n),
            }
        }
        Ok(Self {
            steps,
            repeat: repeat.unwrap_or(1),
        })
    }

    /// The built-in script with this name, if there is one.
    #[must_use]
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            "basic" => Self::parse(BASIC).ok(),
            "pages" => Self::parse(PAGES_DEMO).ok(),
            "interrupt" => Self::parse(INTERRUPT).ok(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
