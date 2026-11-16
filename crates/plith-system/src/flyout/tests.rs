//! Unit tests for the pure flyout layer. No windows, no hooks.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{
    HiddenRegistry, RestoreHandle, SuppressionWindow, WindowFacts, is_host_class, is_orphan,
    should_hide,
};
use crate::test_support::ScratchDir;

/// Any boot stamp; the pure tests only need the writer and reader to agree.
const BOOT: u64 = 1_800_000_000;

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
    let scratch = ScratchDir::new("restore");
    let journal = scratch.journal().to_path_buf();

    let handle = RestoreHandle::new(Some(journal.clone()), BOOT, record_show);
    assert!(handle.hide(7, |_| true, |_| true));
    assert!(handle.hide(9, |_| true, |_| true));
    assert!(!handle.hide(11, |_| false, |_| true));
    assert_eq!(handle.pending(), 2);
    assert_eq!(
        super::journal::read(&journal, BOOT),
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
    let handle = RestoreHandle::new(None, BOOT, |_| {});
    assert!(handle.hide(5, |_| true, |_| true));
    let poisoner = handle.clone();
    let _ = std::thread::spawn(move || poisoner.poison_for_test()).join();
    assert!(handle.is_poisoned_for_test(), "the lock really is poisoned");
    handle.restore_now();
    assert_eq!(handle.pending(), 0);
}

#[test]
fn orphan_needs_the_hide_facts_a_window_and_hidden() {
    assert!(is_orphan(&flyout(), true, false));
    assert!(!is_orphan(&flyout(), false, false), "not a window any more");
    assert!(
        !is_orphan(&flyout(), true, true),
        "visible: not ours to show"
    );
    assert!(
        !is_orphan(&facts("NativeHWNDHost", "SomeApp", Some(0x4)), true, false),
        "a host class in another app's process is a stranger"
    );
    assert!(
        !is_orphan(
            &facts("NativeHWNDHost", "ShellExperienceHost", Some(0x1)),
            true,
            false
        ),
        "a shell window in another band is a stranger"
    );
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

/// A journal left by a killed run is cleared at start. Which handles would be shown is the
/// pure [`is_orphan`] decision, tested above; here the handle is 1 (never a window). Dry run,
/// so nothing is hidden either.
#[cfg(windows)]
#[test]
fn start_clears_a_stale_journal() {
    use super::{FlyoutSuppressor, Mode};

    let scratch = ScratchDir::new("start");
    let path = scratch.journal().to_path_buf();
    std::fs::write(&path, "boot 1\n1\ngarbage\n").expect("stale journal");
    let mut suppressor =
        FlyoutSuppressor::start_with_journal(Mode::DryRun, Some(path.clone())).expect("start");
    assert!(!path.exists(), "the start-up pass clears the journal");
    assert_eq!(suppressor.restore_handle().pending(), 0);
    assert_eq!(suppressor.failure(), None);
    assert_eq!(suppressor.stop(), Ok(()));
}

#[test]
fn hide_is_journaled_before_it_is_posted() {
    let scratch = ScratchDir::new("journal-first");
    let journal = scratch.journal().to_path_buf();
    let handle = RestoreHandle::new(Some(journal.clone()), BOOT, |_| {});
    let seen_at_post = Mutex::new(Vec::new());
    assert!(handle.hide(
        21,
        |_| true,
        |_| {
            *seen_at_post.lock().expect("not poisoned") = super::journal::read(&journal, BOOT);
            true
        }
    ));
    assert_eq!(
        *seen_at_post.lock().expect("not poisoned"),
        vec![21],
        "a kill during the post still finds the handle in the journal"
    );
}

#[test]
fn failed_post_unjournals() {
    let scratch = ScratchDir::new("journal-unpost");
    let journal = scratch.journal().to_path_buf();
    let handle = RestoreHandle::new(Some(journal.clone()), BOOT, |_| {});
    assert!(handle.hide(3, |_| true, |_| true));
    assert!(
        !handle.hide(4, |_| true, |_| false),
        "a refused post hides nothing"
    );
    assert_eq!(handle.pending(), 1);
    assert_eq!(super::journal::read(&journal, BOOT), vec![3]);
    assert!(!handle.hide(5, |_| false, |_| panic!("not posted for a hidden window")));
}

/// The journal's single-owner lock: a second suppressor on the same journal fails and leaves
/// the file alone; once the first is gone the journal can be taken again. Dry run.
#[cfg(windows)]
#[test]
fn second_owner_of_a_journal_fails() {
    use super::{FlyoutSuppressor, Mode};

    let scratch = ScratchDir::new("owner");
    let path = scratch.journal().to_path_buf();
    let first =
        FlyoutSuppressor::start_with_journal(Mode::DryRun, Some(path.clone())).expect("first");
    std::fs::write(&path, "boot 1\n5\n").expect("the first owner's journal");
    let second = FlyoutSuppressor::start_with_journal(Mode::DryRun, Some(path.clone()));
    let error = second.expect_err("a second owner must fail");
    assert!(error.contains("another Plith process"), "{error}");
    assert!(path.exists(), "the second start never touched the journal");
    drop(first);
    let third =
        FlyoutSuppressor::start_with_journal(Mode::DryRun, Some(path.clone())).expect("third");
    assert!(!path.exists(), "the next owner takes the journal over");
    drop(third);
}
