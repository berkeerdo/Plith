//! Unit tests for the pure flyout layer. No windows, no hooks.

use std::time::{Duration, Instant};

use super::{HiddenRegistry, SuppressionWindow, WindowFacts, should_hide};

fn facts(class: &str, process: &str, band: Option<u32>) -> WindowFacts {
    WindowFacts {
        class: class.to_owned(),
        process: process.to_owned(),
        band,
    }
}

fn flyout() -> WindowFacts {
    facts("NativeHWNDHost", "ShellExperienceHost", Some(0x4))
}

#[test]
fn class_process_band_all_required() {
    assert!(should_hide(&flyout(), true));
    assert!(should_hide(
        &facts("XamlExplorerHostIslandWindow", "explorer", Some(0x12)),
        true
    ));
    assert!(!should_hide(
        &facts("Notepad", "ShellExperienceHost", Some(0x4)),
        true
    ));
    assert!(!should_hide(
        &facts("NativeHWNDHost", "chrome", Some(0x4)),
        true
    ));
    assert!(!should_hide(
        &facts("NativeHWNDHost", "ShellExperienceHost", Some(0x1)),
        true
    ));
}

#[test]
fn unknown_band_allowed() {
    assert!(should_hide(
        &facts("NativeHWNDHost", "Explorer", None),
        true
    ));
}

#[test]
fn closed_window_hides_nothing() {
    assert!(!should_hide(&flyout(), false));
}

#[test]
fn window_expires_after_400ms() {
    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    assert!(!window.is_open(t0));
    window.open(t0);
    assert!(window.is_open(t0));
    assert!(window.is_open(t0 + Duration::from_millis(399)));
    assert!(!window.is_open(t0 + Duration::from_millis(401)));
    window.open(t0 + Duration::from_millis(300));
    assert!(window.is_open(t0 + Duration::from_millis(600)));
}

#[test]
fn restore_all_restores_each_once() {
    let mut registry = HiddenRegistry::new();
    registry.hide(7, |_| true);
    registry.hide(7, |_| true);
    registry.hide(9, |_| true);
    registry.hide(11, |_| false);
    assert_eq!(registry.len(), 2);
    let mut shown = Vec::new();
    registry.restore_all(|h| shown.push(h));
    assert_eq!(shown, vec![7, 9]);
}

#[test]
fn restore_after_stop_is_empty() {
    let mut registry = HiddenRegistry::new();
    registry.hide(7, |_| true);
    registry.restore_all(|_| {});
    assert!(registry.is_empty());
    let mut calls = 0;
    registry.restore_all(|_| calls += 1);
    assert_eq!(calls, 0);
}
