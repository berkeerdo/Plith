//! Unit tests for the pure endpoint layer.

use super::*;

fn endpoint(id: &str) -> Endpoint {
    Endpoint {
        id: id.to_string(),
        friendly_name: format!("name of {id}"),
        device_name: format!("device of {id}"),
    }
}

#[test]
fn pin_wins_when_active() {
    let active = [endpoint("a"), endpoint("b")];
    let got = resolve_target(Some("b"), &active, Some("a"));
    assert_eq!(got.as_deref(), Some("b"));
}

#[test]
fn pin_falls_back_when_missing() {
    let active = [endpoint("a")];
    let got = resolve_target(Some("gone"), &active, Some("a"));
    assert_eq!(got.as_deref(), Some("a"));
}

#[test]
fn no_default_no_pin_is_none() {
    assert_eq!(resolve_target(None, &[endpoint("a")], None), None);
    assert_eq!(resolve_target(Some("gone"), &[], None), None);
}

#[test]
fn pin_ids_compare_case_insensitively() {
    let active = [endpoint("{0.0.0.00000000}.{ABC-def}")];
    let got = resolve_target(Some("{0.0.0.00000000}.{abc-DEF}"), &active, None);
    assert_eq!(got.as_deref(), Some("{0.0.0.00000000}.{ABC-def}"));
}

#[test]
fn empty_pin_follows_default() {
    let active = [endpoint("a"), endpoint("b")];
    assert_eq!(
        resolve_target(Some(""), &active, Some("b")).as_deref(),
        Some("b")
    );
}

#[test]
fn state_default_is_empty() {
    let state = EndpointState::default();
    assert!(state.active.is_empty());
    assert_eq!(state.default_id, None);
    assert_eq!(state.attached, None);
}
