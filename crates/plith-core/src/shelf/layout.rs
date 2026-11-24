//! Where the wagons sit: rows of four on tracks, the drop slot after the last wagon, and the
//! hit rectangles. Positions come from `shelfPage()` in `docs/design/r3/ray-paneli.html`
//! (one row, page 214 high); a second row adds one row pitch.

use super::CAPACITY;
use crate::draw::Rect;

/// Wagons per row.
pub const COLUMNS: usize = 4;
/// Page width the layout is made for.
const PAGE_W: f64 = 620.0;
/// Left edge of the first wagon and of the track.
pub const LEFT: f64 = 30.0;
/// Right edge of the track and of the last slot.
pub const RIGHT: f64 = PAGE_W - 30.0;
/// Wagon width.
pub const WAGON_W: f64 = 104.0;
/// Wagon height.
pub const WAGON_H: f64 = 92.0;
/// Gap between wagons.
pub const GAP: f64 = 12.0;
/// Top of the first row of wagons.
pub const TOP: f64 = 62.0;
/// Track y of the first row.
pub const TRACK_Y: f64 = 170.0;
/// Distance from one row to the next.
pub const ROW_PITCH: f64 = 124.0;
/// Page height with one row of wagons (the mock's `h = 214`).
pub const ONE_ROW_HEIGHT: f64 = 214.0;
/// Page height of the empty shelf.
pub const EMPTY_HEIGHT: f64 = 170.0;
/// Distance of a station from the wagon edge.
pub const STATION_INSET: f64 = 20.0;
/// Width and height of the remove cross, drawn.
pub const CROSS_SIZE: f64 = 10.0;
/// Distance of the cross centre from the wagon's top and right edges.
const CROSS_INSET: f64 = 14.0;
/// Width and height of the remove target around the cross.
const CROSS_TARGET: f64 = 24.0;
/// Width the type tag may use: it stops where the remove target starts.
pub const TAG_W: f64 = WAGON_W - CROSS_INSET - CROSS_TARGET / 2.0 - 10.0;
/// Width and height of the undo word's target.
pub const UNDO_W: f64 = 84.0;
/// Width of the clear word's target, at the right end of the footer.
pub const CLEAR_W: f64 = 76.0;
/// Left edge of the clear word.
pub const CLEAR_X: f64 = RIGHT - CLEAR_W;
/// Gap between the undo word and the clear word.
const FOOTER_GAP: f64 = 8.0;
/// Left edge of the undo word, before the clear word.
pub const UNDO_X: f64 = CLEAR_X - FOOTER_GAP - UNDO_W;
/// Left edge of the footer line.
pub const FOOTER_X: f64 = LEFT;
/// The narrowest the drop slot gets: the one at the fifth cell of a row.
pub const SLOT_MIN_W: f64 = RIGHT - (LEFT + 4.0 * (WAGON_W + GAP));
const UNDO_H: f64 = 20.0;

/// One wagon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wagon {
    /// The wagon body.
    pub rect: Rect,
    /// The y of the track under it.
    pub track_y: f64,
    /// The x of its two stations.
    pub stations: [f64; 2],
    /// The centre of the remove cross, in the top right corner of the wagon.
    pub cross: (f64, f64),
    /// The click target that removes it: a square around the cross.
    pub remove: Rect,
}

/// The whole shelf page.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The page height.
    pub height: f64,
    /// The wagons, newest first (the first item is the first wagon).
    pub wagons: Vec<Wagon>,
    /// The drop slot, after the last wagon; `None` for the empty shelf.
    pub slot: Option<Rect>,
    /// The y of each row's track.
    pub tracks: Vec<f64>,
    /// The y of the footer baseline.
    pub footer_y: f64,
    /// The undo word's click target, at the right end of the footer.
    pub undo: Rect,
    /// The clear word's click target, at the right end of the footer.
    pub clear: Rect,
}

fn row_top(row: usize) -> f64 {
    // Allowed: a row index is 0 or 1.
    #[allow(clippy::cast_precision_loss)]
    let row = row as f64;
    row.mul_add(ROW_PITCH, TOP)
}

fn col_x(col: usize) -> f64 {
    // Allowed: a column index is at most 4.
    #[allow(clippy::cast_precision_loss)]
    let col = col as f64;
    col.mul_add(WAGON_W + GAP, LEFT)
}

/// The layout for `count` items, at most [`CAPACITY`] (more are laid out as eight).
#[must_use]
pub fn layout(count: usize) -> Layout {
    let count = count.min(CAPACITY);
    if count == 0 {
        return Layout {
            height: EMPTY_HEIGHT,
            wagons: Vec::new(),
            slot: None,
            tracks: Vec::new(),
            footer_y: 0.0,
            undo: Rect::default(),
            clear: Rect::default(),
        };
    }
    // The slot is the cell after the last wagon: a full row of four keeps it on that row, at
    // the fifth cell, where the mock puts it.
    let slot_row = (count - 1) / COLUMNS;
    let rows = slot_row + 1;
    let wagons = (0..count)
        .map(|i| wagon(i / COLUMNS, i % COLUMNS))
        .collect();
    let slot_col = count - slot_row * COLUMNS;
    let slot_x = col_x(slot_col);
    let slot_w = if slot_col == COLUMNS {
        RIGHT - slot_x
    } else {
        WAGON_W
    };
    #[allow(clippy::cast_precision_loss)]
    let height = (rows - 1) as f64 * ROW_PITCH + ONE_ROW_HEIGHT;
    let footer_y = height - 18.0;
    Layout {
        height,
        wagons,
        slot: Some(Rect {
            x: slot_x,
            y: row_top(slot_row),
            w: slot_w,
            h: WAGON_H,
        }),
        tracks: (0..rows).map(|r| row_top(r) + (TRACK_Y - TOP)).collect(),
        footer_y,
        undo: Rect {
            x: UNDO_X,
            y: footer_y - 12.0,
            w: UNDO_W,
            h: UNDO_H,
        },
        clear: Rect {
            x: CLEAR_X,
            y: footer_y - 12.0,
            w: CLEAR_W,
            h: UNDO_H,
        },
    }
}

fn wagon(row: usize, col: usize) -> Wagon {
    let (x, y) = (col_x(col), row_top(row));
    let track_y = y + (TRACK_Y - TOP);
    let (cx, cy) = (x + WAGON_W - CROSS_INSET, y + CROSS_INSET);
    Wagon {
        rect: Rect {
            x,
            y,
            w: WAGON_W,
            h: WAGON_H,
        },
        track_y,
        stations: [x + STATION_INSET, x + WAGON_W - STATION_INSET],
        cross: (cx, cy),
        remove: Rect {
            x: cx - CROSS_TARGET / 2.0,
            y: cy - CROSS_TARGET / 2.0,
            w: CROSS_TARGET,
            h: CROSS_TARGET,
        },
    }
}

#[cfg(test)]
mod tests;
