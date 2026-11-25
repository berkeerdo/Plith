use super::*;

const OWN: Version = Version::new(0, 4, 0);

fn flow(mode: &Mode, situation: Situation) -> Flow {
    Flow::new(mode, situation, OWN, None)
}

fn fresh() -> Flow {
    flow(&Mode::Install, Situation::Fresh)
}

/// A flow that is installing, past the start screen.
fn installing() -> Flow {
    let mut f = fresh();
    f.on_action(SetupAction::Install);
    f
}

fn failed(step: u8, reason: &str) -> Report {
    Report::Failed {
        step,
        reason: reason.to_owned(),
    }
}

#[test]
fn start_install_elevates() {
    let mut f = fresh();
    assert_eq!(f.screen(), Screen::Start);
    let effects = f.on_action(SetupAction::Install);
    assert_eq!(effects, [Effect::Elevate(Job::Install)]);
    assert_eq!(f.screen(), Screen::Running { done: 0 });
    // A second press while running starts nothing.
    assert!(f.on_action(SetupAction::Install).is_empty());
}

#[test]
fn reports_press_ripples_then_done() {
    let mut f = installing();
    for n in 1..=3 {
        assert!(f.on_report(Report::Step(n)).is_empty());
        assert_eq!(f.screen(), Screen::Running { done: n });
    }
    let effects = f.on_report(Report::Finished);
    assert_eq!(effects, [Effect::SetAutostart(true)]);
    assert_eq!(f.screen(), Screen::Done { autostart: true });
}

#[test]
fn log_reports_become_log_effects() {
    let mut f = installing();
    assert_eq!(
        f.on_report(Report::Log("copied".to_owned())),
        [Effect::Log("copied".to_owned())]
    );
    assert_eq!(f.screen(), Screen::Running { done: 0 });
}

#[test]
fn cancelled_uac_returns_to_start_with_line() {
    let mut f = installing();
    f.on_elevate_failed(true);
    assert_eq!(f.screen(), Screen::UacCancelled);
    // The declined screen offers the install again.
    assert_eq!(
        f.on_action(SetupAction::Install),
        [Effect::Elevate(Job::Install)]
    );
}

#[test]
fn elevation_that_failed_for_another_reason_is_an_error() {
    let mut f = installing();
    f.on_elevate_failed(false);
    assert_eq!(
        f.screen(),
        Screen::Error {
            step: 1,
            code: 0,
            busy: false
        }
    );
}

#[test]
fn failure_shows_error_and_retry_elevates_again() {
    let mut f = installing();
    f.on_report(Report::Step(1));
    let effects = f.on_report(failed(2, "1223"));
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::Log(text) if text.contains("1223")));
    assert_eq!(
        f.screen(),
        Screen::Error {
            step: 2,
            code: 1223,
            busy: false
        }
    );
    assert_eq!(
        f.on_action(SetupAction::Retry),
        [Effect::Elevate(Job::Install)]
    );
    assert_eq!(f.screen(), Screen::Running { done: 0 });
}

#[test]
fn a_reason_that_is_not_a_number_shows_no_code() {
    let mut f = installing();
    f.on_report(failed(1, "disk <full>"));
    assert_eq!(
        f.screen(),
        Screen::Error {
            step: 1,
            code: 0,
            busy: false
        }
    );
}

#[test]
fn worker_exit_without_a_report_is_an_error() {
    let cases = [(2, false, "refused"), (3, true, "another"), (1, false, "1")];
    for (exit, busy, word) in cases {
        let mut f = installing();
        let effects = f.on_worker_exit(exit);
        assert!(
            matches!(&effects[..], [Effect::Log(t)] if t.contains(word)),
            "{exit}"
        );
        assert_eq!(
            f.screen(),
            Screen::Error {
                step: 1,
                code: 0,
                busy
            },
            "{exit}"
        );
    }
}

#[test]
fn worker_exit_after_the_end_changes_nothing() {
    let mut f = installing();
    f.on_report(Report::Finished);
    let done = f.screen();
    assert!(f.on_worker_exit(0).is_empty());
    assert_eq!(f.screen(), done);
    let mut f = installing();
    f.on_report(failed(2, "5"));
    let error = f.screen();
    assert!(f.on_worker_exit(1).is_empty());
    assert_eq!(f.screen(), error);
}

#[test]
fn newer_installed_offers_only_close() {
    let installed = Version::new(0, 9, 0);
    let mut f = flow(&Mode::Install, Situation::NewerInstalled { installed });
    assert_eq!(f.screen(), Screen::Newer { installed });
    assert_eq!(f.primary_action(), Some(SetupAction::Close));
    assert!(f.on_action(SetupAction::Install).is_empty());
    assert!(f.on_action(SetupAction::Retry).is_empty());
    assert_eq!(f.on_action(SetupAction::Close), [Effect::Quit]);
}

#[test]
fn update_mode_shows_versions() {
    let from = Version::new(0, 3, 9);
    let mut f = flow(&Mode::Update { from }, Situation::Upgrade { from });
    assert_eq!(f.screen(), Screen::Update { from, to: OWN });
    assert_eq!(f.primary_action(), Some(SetupAction::Install));
    assert_eq!(
        f.on_action(SetupAction::Install),
        [Effect::Elevate(Job::Install)]
    );
    // Run by hand over an older install, the same screen.
    let older = Version::new(0, 4, 0);
    let f = flow(&Mode::Install, Situation::Upgrade { from: older });
    assert_eq!(
        f.screen(),
        Screen::Update {
            from: older,
            to: OWN
        }
    );
}

#[test]
fn repair_and_over_wpf_start_like_a_fresh_install() {
    for situation in [
        Situation::Repair,
        Situation::OverWpf {
            from: Version::new(0, 3, 1),
        },
    ] {
        assert_eq!(flow(&Mode::Install, situation).screen(), Screen::Start);
    }
}

#[test]
fn uninstall_removes_run_value_and_deletes_data_when_ticked() {
    let mut f = flow(&Mode::Uninstall, Situation::Repair);
    assert_eq!(f.screen(), Screen::UninstallConfirm { delete_data: false });
    assert!(f.on_action(SetupAction::ToggleDeleteData).is_empty());
    assert_eq!(f.screen(), Screen::UninstallConfirm { delete_data: true });
    assert_eq!(
        f.on_action(SetupAction::Uninstall),
        [Effect::Elevate(Job::Uninstall)]
    );
    assert_eq!(f.screen(), Screen::Uninstalling { done: 0 });
    f.on_report(Report::Step(3));
    assert_eq!(f.screen(), Screen::Uninstalling { done: 3 });
    assert_eq!(
        f.on_report(Report::Finished),
        [Effect::SetAutostart(false), Effect::DeleteUserData]
    );
    assert_eq!(f.screen(), Screen::Uninstalled);
    assert_eq!(f.on_action(SetupAction::Close), [Effect::Quit]);
}

#[test]
fn uninstall_keeps_data_when_not_ticked() {
    let mut f = flow(&Mode::Uninstall, Situation::Repair);
    f.on_action(SetupAction::Uninstall);
    assert_eq!(f.on_report(Report::Finished), [Effect::SetAutostart(false)]);
}

#[test]
fn uninstall_failure_retries_the_uninstall() {
    let mut f = flow(&Mode::Uninstall, Situation::Repair);
    f.on_action(SetupAction::Uninstall);
    f.on_report(failed(3, "32"));
    assert_eq!(
        f.screen(),
        Screen::Error {
            step: 3,
            code: 32,
            busy: false
        }
    );
    assert_eq!(
        f.on_action(SetupAction::Retry),
        [Effect::Elevate(Job::Uninstall)]
    );
    assert_eq!(f.screen(), Screen::Uninstalling { done: 0 });
}

#[test]
fn declined_uninstall_prompt_returns_to_the_question() {
    let mut f = flow(&Mode::Uninstall, Situation::Repair);
    f.on_action(SetupAction::ToggleDeleteData);
    f.on_action(SetupAction::Uninstall);
    f.on_elevate_failed(true);
    assert_eq!(f.screen(), Screen::UninstallConfirm { delete_data: true });
}

#[test]
fn done_open_starts_plith_and_quits() {
    let mut f = installing();
    f.on_report(Report::Finished);
    assert_eq!(f.primary_action(), Some(SetupAction::Open));
    assert_eq!(
        f.on_action(SetupAction::Open),
        [Effect::StartPlith, Effect::Quit]
    );
}

#[test]
fn done_toggle_writes_at_once_and_close_applies_nothing_more() {
    let mut f = installing();
    f.on_report(Report::Finished);
    assert_eq!(
        f.on_action(SetupAction::ToggleAutostart),
        [Effect::SetAutostart(false)]
    );
    assert_eq!(f.screen(), Screen::Done { autostart: false });
    assert_eq!(
        f.on_action(SetupAction::ToggleAutostart),
        [Effect::SetAutostart(true)]
    );
    assert_eq!(f.on_action(SetupAction::Close), [Effect::Quit]);
}

#[test]
fn close_is_ignored_while_running() {
    let mut f = installing();
    assert!(f.on_action(SetupAction::Close).is_empty());
    assert_eq!(f.primary_action(), None);
    let mut f = flow(&Mode::Uninstall, Situation::Repair);
    f.on_action(SetupAction::Uninstall);
    assert!(f.on_action(SetupAction::Close).is_empty());
    assert_eq!(f.primary_action(), None);
}

#[test]
fn actions_of_other_screens_do_nothing() {
    let mut f = fresh();
    for action in [
        SetupAction::Open,
        SetupAction::Retry,
        SetupAction::Uninstall,
        SetupAction::ToggleAutostart,
        SetupAction::ToggleDeleteData,
        SetupAction::Drag,
    ] {
        assert!(f.on_action(action).is_empty(), "{action:?}");
        assert_eq!(f.screen(), Screen::Start, "{action:?}");
    }
}

#[test]
fn primary_action_of_every_screen() {
    let mut f = fresh();
    assert_eq!(f.primary_action(), Some(SetupAction::Install));
    f.on_elevate_failed(true);
    assert_eq!(f.primary_action(), Some(SetupAction::Install));
    f.on_action(SetupAction::Install);
    f.on_report(failed(1, "5"));
    assert_eq!(f.primary_action(), Some(SetupAction::Retry));
    let f = flow(&Mode::Uninstall, Situation::Repair);
    assert_eq!(f.primary_action(), Some(SetupAction::Uninstall));
}

/// A flow over `situation` with the Run value found at start, taken through to the done screen.
fn finished(mode: &Mode, situation: Situation, run: Option<bool>) -> (Screen, Vec<Effect>) {
    let mut f = Flow::new(mode, situation, OWN, run);
    f.on_action(SetupAction::Install);
    let effects = f.on_report(Report::Finished);
    (f.screen(), effects)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the tuple is a throwaway result"
)]
fn assert_tick(got: (Screen, Vec<Effect>), on: bool) {
    assert_eq!(got.0, Screen::Done { autostart: on });
    assert_eq!(got.1, [Effect::SetAutostart(on)]);
}

#[test]
fn update_keeps_autostart_off_when_no_run_value() {
    let from = Version::new(0, 3, 9);
    let situation = Situation::Upgrade { from };
    assert_tick(finished(&Mode::Update { from }, situation, None), false);
    assert_tick(finished(&Mode::Install, situation, Some(false)), false);
    assert_tick(
        finished(&Mode::Update { from }, situation, Some(true)),
        true,
    );
}

#[test]
fn repair_keeps_autostart_on_when_ours() {
    assert_tick(
        finished(&Mode::Install, Situation::Repair, Some(true)),
        true,
    );
    assert_tick(finished(&Mode::Install, Situation::Repair, None), false);
}

#[test]
fn fresh_install_ticks_autostart() {
    for run in [None, Some(true), Some(false)] {
        assert_tick(finished(&Mode::Install, Situation::Fresh, run), true);
    }
}

#[test]
fn over_wpf_ticks_autostart() {
    let situation = Situation::OverWpf {
        from: Version::new(0, 3, 1),
    };
    for run in [None, Some(true)] {
        assert_tick(finished(&Mode::Install, situation, run), true);
    }
}
