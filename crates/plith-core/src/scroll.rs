//! The one-shot title scroll: hold, travel left, hold, travel back, rest. Pure timing; the
//! painter turns an offset into pixels.

/// Seconds to hold at each end of the travel.
pub const SCROLL_HOLD_S: f64 = 0.8;
/// Travel speed in DIP per second.
pub const SCROLL_SPEED: f64 = 40.0;

/// The horizontal offset (0 at rest, negative while moving) `elapsed_s` seconds after the start
/// for text that overflows its box by `overflow` DIP. Negative elapsed gives 0.
#[must_use]
pub fn scroll_offset(elapsed_s: f64, overflow: f64) -> f64 {
    if elapsed_s.is_nan() || elapsed_s <= 0.0 || overflow.is_nan() || overflow <= 0.0 {
        return 0.0;
    }
    let travel = overflow / SCROLL_SPEED;
    let out_at = SCROLL_HOLD_S;
    let far_at = out_at + travel;
    let back_at = far_at + SCROLL_HOLD_S;
    let home_at = back_at + travel;
    if elapsed_s < out_at {
        0.0
    } else if elapsed_s < far_at {
        -(elapsed_s - out_at) * SCROLL_SPEED
    } else if elapsed_s < back_at {
        -overflow
    } else if elapsed_s < home_at {
        -overflow + (elapsed_s - back_at) * SCROLL_SPEED
    } else {
        0.0
    }
}

/// The seconds a scroll of `overflow` DIP takes from start to rest; 0 without overflow.
#[must_use]
pub fn scroll_run_s(overflow: f64) -> f64 {
    if overflow.is_nan() || overflow <= 0.0 {
        return 0.0;
    }
    2.0 * SCROLL_HOLD_S + 2.0 * overflow / SCROLL_SPEED
}

#[cfg(test)]
mod tests;
