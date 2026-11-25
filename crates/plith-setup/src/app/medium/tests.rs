use super::{Plan, command_line, plan};

#[test]
fn a_medium_window_goes_on_without_asking_the_shell() {
    let plan = plan(Ok(false), || panic!("the shell is not asked at medium"));
    assert_eq!(plan, Plan::Medium);
}

#[test]
fn an_elevated_window_under_a_medium_shell_starts_again() {
    assert_eq!(plan(Ok(true), || Ok(false)), Plan::Relaunch);
}

#[test]
fn an_elevated_shell_or_an_unknown_level_keeps_the_window_elevated() {
    // Relaunching through an elevated shell would start an elevated copy again, forever.
    assert!(matches!(plan(Ok(true), || Ok(true)), Plan::Elevated(_)));
    assert!(matches!(
        plan(Ok(true), || Err("no shell".to_string())),
        Plan::Elevated(_)
    ));
    assert!(matches!(
        plan(Err("no token".to_string()), || Ok(false)),
        Plan::Elevated(_)
    ));
}

#[test]
fn the_same_arguments_are_passed_on() {
    let args = ["--update", "--from", "0.3.1"].map(String::from);
    assert_eq!(command_line(&args), "--update --from 0.3.1");
    assert_eq!(command_line(&[]), "");
    assert_eq!(command_line(&["a b".to_string()]), "\"a b\"");
}
