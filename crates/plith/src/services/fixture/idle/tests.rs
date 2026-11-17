use super::*;

#[test]
fn the_mock_has_five_plays_and_two_days_of_hours() {
    assert_eq!(plays((2026, 11, 17)).in_window(0, 1440).len(), 5);
    let read_at = Stamp {
        day: (2026, 12, 31),
        hour: 9,
        minute: 0,
    };
    let f = forecast((2026, 12, 31), read_at);
    assert_eq!(f.hours.len(), 48);
    assert_eq!(f.hours[24].day, (2027, 1, 1));
}
