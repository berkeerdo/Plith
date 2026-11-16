//! Unit tests for panic containment. `resume_unwind` starts a real unwind but skips the panic
//! hook, so these tests panic without printing and without touching the global hook.

use std::panic::resume_unwind;

use super::{FailureSlot, contain, contain_text};

#[test]
fn contain_reports_a_panic() {
    assert!(!contain(|| resume_unwind(Box::new("boom"))));
    assert_eq!(
        contain_text(|| resume_unwind(Box::new(String::from("owned text")))),
        Some("owned text".to_owned())
    );
    assert_eq!(
        contain_text(|| resume_unwind(Box::new("static text"))),
        Some("static text".to_owned())
    );
    assert_eq!(
        contain_text(|| resume_unwind(Box::new(7_u8))),
        Some("panic with a non-text payload".to_owned())
    );
}

#[test]
fn contain_passes_a_normal_run() {
    let mut ran = false;
    assert!(contain(|| ran = true));
    assert!(ran);
    assert_eq!(contain_text(|| {}), None);
}

#[test]
fn failure_slot_keeps_the_first_text() {
    let slot = FailureSlot::default();
    assert_eq!(slot.get(), None);
    let shared = slot.clone();
    shared.record("first".to_owned());
    slot.record("second".to_owned());
    assert_eq!(slot.get(), Some("first".to_owned()));
    assert_eq!(shared.get(), Some("first".to_owned()));
}
