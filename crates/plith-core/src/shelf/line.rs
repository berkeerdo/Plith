//! The `drag` log lines: one per drag, in the style of the `motion` line.

use std::fmt::Write as _;

/// How a drag into the island ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InEnd {
    /// The files were dropped.
    Dropped,
    /// The pointer left without a drop.
    Left,
    /// The data had no file list (text, a link, an elevated source): nothing opened.
    Refused,
}

impl InEnd {
    fn word(self) -> &'static str {
        match self {
            InEnd::Dropped => "drop",
            InEnd::Left => "left",
            InEnd::Refused => "refused",
        }
    }
}

/// A drag into the island.
#[derive(Debug, Clone, PartialEq)]
pub struct DragIn {
    /// The clipboard formats the data object offered, as names.
    pub formats: String,
    /// How many files the drop carried (0 when nothing was dropped).
    pub files: usize,
    /// How the drag ended.
    pub end: InEnd,
    /// Milliseconds from the first hover to the shelf opening, when it opened.
    pub open_ms: Option<u32>,
    /// The effect granted at the first enter: `copy`, `link` or `none`.
    pub effect: &'static str,
    /// The island DIP point of the first enter.
    pub point: (i32, i32),
    /// How many times the drag entered the island (a leave and a return inside the grace
    /// counts again): the chatter of the drag.
    pub enters: u32,
}

impl DragIn {
    /// `drag: dir=in end=drop files=3 effect=copy point=310,12 enters=1 open_ms=152 formats=...`
    #[must_use]
    pub fn line(&self) -> String {
        let mut out = format!(
            "drag: dir=in end={} files={} effect={} point={},{} enters={}",
            self.end.word(),
            self.files,
            self.effect,
            self.point.0,
            self.point.1,
            self.enters
        );
        match self.open_ms {
            Some(ms) => {
                let _ = write!(out, " open_ms={ms}");
            }
            None => out.push_str(" open_ms=none"),
        }
        let _ = write!(out, " formats={}", self.formats);
        out
    }
}

/// A drag out of the island.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragOut {
    /// How many files were offered.
    pub items: usize,
    /// How many wagons were skipped because the file was gone.
    pub missing: usize,
    /// What the drag ended as: `copy`, `link`, `none` (cancelled or refused) or `error`.
    pub effect: &'static str,
    /// How long the drag lasted, in milliseconds.
    pub ms: u32,
}

impl DragOut {
    /// `drag: dir=out items=2 missing=0 effect=copy ms=840`
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "drag: dir=out items={} missing={} effect={} ms={}",
            self.items, self.missing, self.effect, self.ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_in_line_names_every_fact() {
        let line = DragIn {
            formats: "hdrop,FileNameW".into(),
            files: 3,
            end: InEnd::Dropped,
            open_ms: Some(152),
            effect: "copy",
            point: (310, 12),
            enters: 2,
        }
        .line();
        assert_eq!(
            line,
            "drag: dir=in end=drop files=3 effect=copy point=310,12 enters=2 open_ms=152 \
             formats=hdrop,FileNameW"
        );
    }

    #[test]
    fn a_refused_drag_has_no_open_time() {
        let line = DragIn {
            formats: "text".into(),
            files: 0,
            end: InEnd::Refused,
            open_ms: None,
            effect: "none",
            point: (0, 0),
            enters: 1,
        }
        .line();
        assert!(line.contains("end=refused") && line.contains("open_ms=none"));
    }

    #[test]
    fn the_out_line_names_every_fact() {
        let line = DragOut {
            items: 2,
            missing: 1,
            effect: "copy",
            ms: 840,
        }
        .line();
        assert_eq!(line, "drag: dir=out items=2 missing=1 effect=copy ms=840");
    }
}
