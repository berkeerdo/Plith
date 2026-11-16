//! Unit tests for the pure flyout layer. No windows, no hooks.

use std::time::{Duration, Instant};

use std::sync::Mutex;

use super::{
    HiddenRegistry, RestoreHandle, SuppressionWindow, WindowFacts, is_host_class, should_hide,
};

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
    assert!(registry.hide(7, |_| true, |_| true));
    assert!(registry.hide(7, |_| true, |_| true));
    assert!(registry.hide(9, |_| true, |_| true));
    // Already hidden by somebody else: not ours to show.
    assert!(!registry.hide(11, |_| false, |_| panic!("must not hide")));
    // The request was not accepted: nothing is owed.
    assert!(!registry.hide(13, |_| true, |_| false));
    assert_eq!(registry.len(), 2);
    let mut shown = Vec::new();
    registry.restore_all(|h| shown.push(h));
    assert_eq!(shown, vec![7, 9]);
}

#[test]
fn restore_after_stop_is_empty() {
    let mut registry = HiddenRegistry::new();
    registry.hide(7, |_| true, |_| true);
    registry.restore_all(|_| {});
    assert!(registry.is_empty());
    let mut calls = 0;
    registry.restore_all(|_| calls += 1);
    assert_eq!(calls, 0);
}

/// What `record_show` was called with; only `restore_now_through_a_shared_handle_empties_the_set`
/// uses it, so parallel tests do not share it.
static SHOWN: Mutex<Vec<isize>> = Mutex::new(Vec::new());

fn record_show(hwnd: isize) {
    SHOWN.lock().expect("not poisoned").push(hwnd);
}

#[test]
fn restore_now_through_a_shared_handle_empties_the_set() {
    let dir = std::env::temp_dir().join(format!("plith-restore-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp folder");
    let journal = dir.join("flyout-hidden.txt");
    let _ = std::fs::remove_file(&journal);

    let handle = RestoreHandle::new(Some(journal.clone()), record_show);
    assert!(handle.hide(7, |_| true, |_| true));
    assert!(handle.hide(9, |_| true, |_| true));
    assert!(!handle.hide(11, |_| false, |_| true));
    assert_eq!(handle.pending(), 2);
    assert_eq!(
        super::journal::read(&journal),
        vec![7, 9],
        "each hide is journaled"
    );

    let other = handle.clone();
    std::thread::spawn(move || other.restore_now())
        .join()
        .expect("restore thread");
    assert_eq!(*SHOWN.lock().expect("not poisoned"), vec![7, 9]);
    assert_eq!(
        handle.pending(),
        0,
        "the set is shared, so the original sees it empty"
    );
    assert!(!journal.exists(), "restore_now clears the journal");

    handle.restore_now();
    assert_eq!(SHOWN.lock().expect("not poisoned").len(), 2, "idempotent");
}

#[test]
fn restore_now_tolerates_a_poisoned_lock() {
    let handle = RestoreHandle::new(None, |_| {});
    assert!(handle.hide(5, |_| true, |_| true));
    let poisoner = handle.clone();
    let _ = std::thread::spawn(move || poisoner.poison_for_test()).join();
    handle.restore_now();
    assert_eq!(handle.pending(), 0);
}

#[test]
fn host_class_check_matches_exactly_without_allocating() {
    let utf16 = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    for class in super::HOST_CLASSES {
        assert!(is_host_class(&utf16(class)), "{class}");
    }
    assert!(!is_host_class(&utf16("Notepad")));
    assert!(!is_host_class(&utf16("NativeHWNDHos")));
    assert!(!is_host_class(&utf16("NativeHWNDHostX")));
    assert!(
        !is_host_class(&utf16("nativehwndhost")),
        "class names are case sensitive"
    );
    assert!(!is_host_class(&[]));
}

/// Starts a dry run suppressor, stops it and proves the thread joined (stop is `Ok`) and the
/// channel closed (the state with the sender died on that thread). Never hides anything.
#[cfg(windows)]
#[test]
fn dry_run_start_stop_joins_and_disconnects() {
    use std::sync::mpsc::TryRecvError;

    use super::{FlyoutSuppressor, Mode};

    let mut suppressor = FlyoutSuppressor::start(Mode::DryRun).expect("hooks install");
    assert_eq!(suppressor.stop(), Ok(()));
    assert_eq!(suppressor.stop(), Ok(()), "stopping twice is fine");
    let end = loop {
        match suppressor.matches().try_recv() {
            Ok(_) => {}
            Err(err) => break err,
        }
    };
    assert_eq!(end, TryRecvError::Disconnected);
}

/// A journal left by a killed run is consumed at start: a handle that names no window (here
/// 1, never a valid window handle) fails the orphan check, so nothing is shown, and the file is
/// cleared. Dry run, so nothing is hidden either.
#[cfg(windows)]
#[test]
fn start_consumes_a_stale_journal_without_showing_strangers() {
    use super::{FlyoutSuppressor, Mode, journal};

    let dir = std::env::temp_dir().join(format!("plith-start-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp folder");
    let path = dir.join("flyout-hidden.txt");
    std::fs::write(&path, "1\ngarbage\n").expect("stale journal");
    let mut suppressor =
        FlyoutSuppressor::start_with_journal(Mode::DryRun, Some(path.clone())).expect("start");
    assert!(!path.exists(), "the start-up pass clears the journal");
    assert_eq!(journal::read(&path), Vec::<isize>::new());
    assert_eq!(suppressor.restore_handle().pending(), 0);
    assert_eq!(suppressor.failure(), None);
    assert_eq!(suppressor.stop(), Ok(()));
}
