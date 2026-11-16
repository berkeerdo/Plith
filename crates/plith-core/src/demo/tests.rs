use super::*;
use crate::shapes::PAGES;

#[test]
fn builtins_parse_and_page_indices_are_in_range() {
    for name in ["basic", "interrupt"] {
        let script = DemoScript::builtin(name).expect("builtin exists");
        assert_eq!(script.repeat, 3, "{name}");
        assert!(!script.steps.is_empty(), "{name}");
        for step in &script.steps {
            if let DemoStep::Page(page) = step {
                assert!(*page < PAGES.len(), "{name}");
            }
        }
    }
    assert_eq!(DemoScript::builtin("nope"), None);
}

#[test]
fn basic_matches_the_pinned_script() {
    let script = DemoScript::builtin("basic").unwrap();
    assert_eq!(
        script.steps,
        vec![
            DemoStep::Expand,
            DemoStep::Wait(1500),
            DemoStep::Page(1),
            DemoStep::Wait(1200),
            DemoStep::Page(2),
            DemoStep::Wait(1200),
            DemoStep::Page(0),
            DemoStep::Wait(1200),
            DemoStep::Collapse,
            DemoStep::Wait(1500),
        ]
    );
}

#[test]
fn parse_trims_whitespace_and_ignores_empty_steps() {
    let script = DemoScript::parse(" expand ;; wait  5 ; collapse ; ").unwrap();
    assert_eq!(
        script.steps,
        vec![DemoStep::Expand, DemoStep::Wait(5), DemoStep::Collapse]
    );
    assert_eq!(script.repeat, 1);
}

#[test]
fn repeat_must_be_last_and_page_index_must_exist() {
    assert!(matches!(
        DemoScript::parse("repeat 2; expand"),
        Err(DemoError::RepeatNotLast(_))
    ));
    assert!(matches!(
        DemoScript::parse("page 4"),
        Err(DemoError::PageOutOfRange(_))
    ));
    assert!(DemoScript::parse("page 3").is_ok());
}

#[test]
fn repeat_must_be_at_least_one_and_numbers_must_parse() {
    assert!(matches!(
        DemoScript::parse("expand; repeat 0"),
        Err(DemoError::BadNumber(_))
    ));
    assert!(matches!(
        DemoScript::parse("wait soon"),
        Err(DemoError::BadNumber(_))
    ));
    assert!(matches!(
        DemoScript::parse("wait"),
        Err(DemoError::BadNumber(_))
    ));
}

#[test]
fn unknown_steps_carry_their_text() {
    let error = DemoScript::parse("expand; dance").unwrap_err();
    assert_eq!(error, DemoError::UnknownStep("dance".to_string()));
    assert!(error.to_string().contains("dance"));
}

#[test]
fn pages_demo_parses() {
    let script = DemoScript::builtin("pages").expect("builtin exists");
    assert_eq!(script.repeat, 3);
    // Every pass starts from the launched content, so each pass measures the same swaps.
    assert_eq!(
        script.steps,
        vec![
            DemoStep::Content(0),
            DemoStep::Expand,
            DemoStep::Wait(1500),
            DemoStep::Page(1),
            DemoStep::Wait(1200),
            DemoStep::Page(2),
            DemoStep::Wait(1200),
            DemoStep::Page(3),
            DemoStep::Wait(1200),
            DemoStep::Page(0),
            DemoStep::Wait(600),
            DemoStep::Content(1),
            DemoStep::Wait(900),
            DemoStep::Page(1),
            DemoStep::Wait(200),
            DemoStep::Content(2),
            DemoStep::Wait(1300),
            DemoStep::Collapse,
            DemoStep::Wait(1500),
        ]
    );
}

#[test]
fn content_step_parses_zero_to_three() {
    for n in 0..=3_u8 {
        let script = DemoScript::parse(&format!("content {n}")).unwrap();
        assert_eq!(script.steps, vec![DemoStep::Content(n)]);
    }
}

#[test]
fn content_step_out_of_range_rejected() {
    assert!(matches!(
        DemoScript::parse("content 4"),
        Err(DemoError::BadNumber(_))
    ));
    assert!(matches!(
        DemoScript::parse("content 300"),
        Err(DemoError::BadNumber(_))
    ));
    assert!(matches!(
        DemoScript::parse("content"),
        Err(DemoError::BadNumber(_))
    ));
    assert!(matches!(
        DemoScript::parse("content x"),
        Err(DemoError::BadNumber(_))
    ));
}
