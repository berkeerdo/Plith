//! The report the elevated worker sends to setup, one message per pipe write.
//!
//! Text format: `S<n>` step, `L<text>` log, `F<n>:<reason>` failure, `D` finished.

/// The longest log or reason text, in bytes.
const MAX_TEXT: usize = 200;

/// One progress message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// A step (1 to 3) has started.
    Step(u8),
    /// A log line.
    Log(String),
    /// A step failed.
    Failed {
        /// The step (1 to 3).
        step: u8,
        /// Why, in short.
        reason: String,
    },
    /// All done.
    Finished,
}

fn cap(text: &str) -> &str {
    if text.len() <= MAX_TEXT {
        return text;
    }
    let mut end = MAX_TEXT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Encodes a report for one pipe write; long texts are cut at 200 bytes.
#[must_use]
pub fn encode(report: &Report) -> Vec<u8> {
    match report {
        Report::Step(n) => format!("S{n}"),
        Report::Log(text) => format!("L{}", cap(text)),
        Report::Failed { step, reason } => format!("F{step}:{}", cap(reason)),
        Report::Finished => "D".to_string(),
    }
    .into_bytes()
}

fn step(text: &str) -> Option<u8> {
    let n: u8 = text.parse().ok()?;
    (1..=3).contains(&n).then_some(n)
}

/// Decodes one message; garbage and steps outside 1 to 3 give `None`.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<Report> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut chars = text.chars();
    let tag = chars.next()?;
    let rest = chars.as_str();
    match tag {
        'S' => step(rest).map(Report::Step),
        'L' if rest.len() <= MAX_TEXT => Some(Report::Log(rest.to_string())),
        'F' => {
            let (n, reason) = rest.split_once(':')?;
            if reason.len() > MAX_TEXT {
                return None;
            }
            Some(Report::Failed {
                step: step(n)?,
                reason: reason.to_string(),
            })
        }
        'D' if rest.is_empty() => Some(Report::Finished),
        _ => None,
    }
}
