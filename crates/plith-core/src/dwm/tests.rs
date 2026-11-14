use super::*;

#[test]
fn delta_subtracts_each_counter() {
    let a = DwmSample {
        refreshes: 100,
        missed: 2,
        dropped: 1,
    };
    let b = DwmSample {
        refreshes: 290,
        missed: 5,
        dropped: 1,
    };
    let d = a.delta(b);
    assert_eq!((d.refreshes, d.missed, d.dropped), (190, 3, 0));
}

#[test]
fn delta_saturates_on_counter_reset() {
    let a = DwmSample {
        refreshes: 500,
        missed: 9,
        dropped: 4,
    };
    let b = DwmSample {
        refreshes: 10,
        missed: 0,
        dropped: 0,
    };
    let d = a.delta(b);
    assert_eq!((d.refreshes, d.missed, d.dropped), (0, 0, 0));
}
