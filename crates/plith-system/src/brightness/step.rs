//! Brightness arithmetic. Pure.

/// The level one step from `current`, saturating at 0 and 100.
#[must_use]
pub fn step_level(current: u8, delta: i8) -> u8 {
    current.min(100).saturating_add_signed(delta).min(100)
}

/// Converts a device's raw `min..=max` reading to a percentage; an empty range reads as 0.
#[must_use]
pub fn raw_to_percent(min: u32, current: u32, max: u32) -> u8 {
    let span = u64::from(max.saturating_sub(min));
    if span == 0 {
        return 0;
    }
    let offset = u64::from(current.clamp(min, max) - min);
    let percent = (offset * 100 + span / 2) / span;
    u8::try_from(percent.min(100)).unwrap_or(100)
}

/// Converts a percentage to a raw value within `min..=max`.
#[must_use]
pub fn percent_to_raw(min: u32, max: u32, percent: u8) -> u32 {
    let span = u64::from(max.saturating_sub(min));
    let offset = (span * u64::from(percent.min(100)) + 50) / 100;
    // `offset <= span <= u32::MAX`, so the conversion cannot fail.
    min + u32::try_from(offset).unwrap_or(u32::MAX - min)
}
