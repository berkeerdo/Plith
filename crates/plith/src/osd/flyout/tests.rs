use super::{Change, change, match_line, mode_name, restore_now};
use plith_core::osd::policy::SuppressMode::{DryRun, Hide};
use plith_system::flyout::{Match, WindowFacts};

#[test]
fn nothing_changes_when_running_matches_wanted() {
    assert_eq!(change(None, None, false), Change::Keep);
    assert_eq!(change(Some(Hide), Some(Hide), false), Change::Keep);
    assert_eq!(change(Some(DryRun), Some(DryRun), false), Change::Keep);
}

#[test]
fn starts_stops_and_switches() {
    assert_eq!(change(None, Some(Hide), false), Change::Start(Hide));
    assert_eq!(change(Some(Hide), None, false), Change::Stop);
    assert_eq!(
        change(Some(DryRun), Some(Hide), false),
        Change::Restart(Hide)
    );
}

#[test]
fn a_pinned_session_only_stops() {
    assert_eq!(change(None, Some(Hide), true), Change::Keep);
    assert_eq!(change(None, Some(DryRun), true), Change::Keep);
    assert_eq!(change(Some(Hide), Some(Hide), true), Change::Stop);
}

#[test]
fn match_lines_say_whether_the_window_was_hidden() {
    let mut found = Match {
        hwnd: 0x1a2b,
        facts: WindowFacts {
            class: String::from("XamlExplorerHostIslandWindow"),
            process: String::from("ShellExperienceHost"),
            band: Some(2),
        },
        hidden: true,
    };
    assert_eq!(
        match_line(&found),
        "osd: flyout hid: XamlExplorerHostIslandWindow of ShellExperienceHost (band 2, hwnd 0x1a2b)"
    );
    found.hidden = false;
    found.facts.band = None;
    assert_eq!(
        match_line(&found),
        "osd: flyout matched (not hidden): XamlExplorerHostIslandWindow of ShellExperienceHost \
         (band unknown, hwnd 0x1a2b)"
    );
}

#[test]
fn mode_names() {
    assert_eq!(mode_name(Hide), "hide");
    assert_eq!(mode_name(DryRun), "dry run");
}

#[test]
fn restore_now_without_a_suppressor_is_a_no_op() {
    restore_now();
    restore_now();
}
