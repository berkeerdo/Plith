use super::Coalesce;

#[test]
fn arm_once_until_cleared() {
    let c = Coalesce::default();
    assert!(c.arm());
    assert!(!c.arm());
    c.clear();
    assert!(c.arm());
}

#[test]
fn failed_post_clears() {
    let c = Coalesce::default();
    assert!(!c.post_with(|| false));
    assert!(c.arm(), "a failed post leaves the coalescer clear");
}

#[test]
fn successful_post_stays_armed_and_coalesces() {
    let c = Coalesce::default();
    assert!(c.post_with(|| true));
    let mut called = false;
    assert!(!c.post_with(|| {
        called = true;
        true
    }));
    assert!(!called, "no second post while one is queued");
}
