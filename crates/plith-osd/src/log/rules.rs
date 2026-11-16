//! The pure rules of the helper log: line text, escaping, the size cap and the path check.

use std::borrow::Cow;
use std::fmt::Write as _;

/// The most bytes one run appends, and the size past which the old log is rotated at open.
pub const LOG_CAP: u64 = 1024 * 1024;

/// The line written once when a run reaches [`LOG_CAP`].
pub const CAP_NOTICE: &str = "log cap reached, later lines are dropped";

/// A local time, split into the fields the line prefix needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// Year, four digits.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u16,
    /// Day of the month.
    pub day: u16,
    /// Hour, 0 to 23.
    pub hour: u16,
    /// Minute.
    pub minute: u16,
    /// Second.
    pub second: u16,
    /// Millisecond.
    pub millis: u16,
}

/// Formats one log line: `YYYY-MM-DD HH:MM:SS.mmm <text>` and a newline. Control characters
/// in `text` are escaped (see [`escape_controls`]), so one call is always one line.
#[must_use]
pub fn format_line(s: Stamp, text: &str) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {}\n",
        s.year,
        s.month,
        s.day,
        s.hour,
        s.minute,
        s.second,
        s.millis,
        escape_controls(text)
    )
}

/// Replaces every control character and Unicode line or paragraph separator with its
/// `\u{..}` escape, so text from outside (a `--pipe` argument, a label) cannot forge lines.
#[must_use]
pub fn escape_controls(text: &str) -> Cow<'_, str> {
    let unsafe_char = |c: char| c.is_control() || c == '\u{2028}' || c == '\u{2029}';
    if !text.chars().any(unsafe_char) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if unsafe_char(c) {
            let _ = write!(out, "{}", c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// What happens to a line of `len` bytes after `written` bytes in this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFate {
    /// Append it.
    Write,
    /// Append [`CAP_NOTICE`] instead, once.
    Notice,
    /// Drop it.
    Drop,
}

/// The cap rule: lines are written while they fit in [`LOG_CAP`]; the first that does not
/// becomes the notice; after the notice (`capped`) every line is dropped.
#[must_use]
pub fn line_fate(written: u64, len: u64, capped: bool) -> LineFate {
    if capped {
        LineFate::Drop
    } else if written.saturating_add(len) <= LOG_CAP {
        LineFate::Write
    } else {
        LineFate::Notice
    }
}

/// Whether a log of `size` bytes left by an earlier run is moved aside at open.
#[must_use]
pub fn rotate_at_open(size: u64) -> bool {
    size > LOG_CAP
}

/// Whether `final_path`, from `GetFinalPathNameByHandleW` (with or without the `\\?\`
/// prefix), names `expected`. Case does not count (NTFS names are case blind); anything
/// else, such as a junction or a link on the way, makes it a different path.
#[must_use]
pub fn same_final_path(final_path: &str, expected: &str) -> bool {
    let strip = |path: &str| -> String {
        let path = path.strip_prefix(r"\\?\").unwrap_or(path);
        path.trim_end_matches('\\').to_lowercase()
    };
    strip(final_path) == strip(expected)
}

#[cfg(test)]
mod tests;
