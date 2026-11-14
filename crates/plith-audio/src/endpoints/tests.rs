//! Unit tests for the pure endpoint layer.

use super::*;
use windows::Win32::Media::Audio::{eCommunications, eConsole, eMultimedia};

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

#[test]
fn volume_is_clamped_to_unit_range() {
    assert_eq!(sanitize_volume(0.5), Some(0.5));
    assert_eq!(sanitize_volume(0.0), Some(0.0));
    assert_eq!(sanitize_volume(1.0), Some(1.0));
    assert_eq!(sanitize_volume(-0.3), Some(0.0));
    assert_eq!(sanitize_volume(7.0), Some(1.0));
    assert_eq!(sanitize_volume(f32::NEG_INFINITY), Some(0.0));
    assert_eq!(sanitize_volume(f32::INFINITY), Some(1.0));
}

#[test]
fn nan_volume_is_rejected() {
    assert_eq!(sanitize_volume(f32::NAN), None);
}

#[test]
fn mute_flips_the_value_that_was_read() {
    assert!(next_mute(false));
    assert!(!next_mute(true));
}

#[test]
fn only_console_and_multimedia_are_written() {
    assert_eq!(DEFAULT_ROLES, [eConsole, eMultimedia]);
    assert!(!DEFAULT_ROLES.contains(&eCommunications));
}
