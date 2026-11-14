use super::*;
use crate::demo::{DemoScript, DemoStep};

fn parse(args: &[&str]) -> Result<Launch, LaunchError> {
    Launch::parse(args.iter().map(|a| (*a).to_string()))
}

#[test]
fn defaults_have_no_demo_and_an_auto_clock() {
    let launch = parse(&[]).unwrap();
    assert_eq!(launch.demo, None);
    assert_eq!(launch.idle_check_secs, None);
    assert_eq!(launch.clock, ClockRequest::Auto);
    assert!(!launch.exit_after_demo);
}

#[test]
fn parse_reads_all_flags() {
    let launch = parse(&[
        "--demo",
        "basic",
        "--idle-check",
        "30",
        "--clock",
        "dxgi",
        "--exit-after-demo",
    ])
    .unwrap();
    assert_eq!(launch.demo, DemoScript::builtin("basic"));
    assert_eq!(launch.idle_check_secs, Some(30));
    assert_eq!(launch.clock, ClockRequest::Dxgi);
    assert!(launch.exit_after_demo);
}

#[test]
fn demo_accepts_an_inline_script() {
    let launch = parse(&["--demo", "expand; wait 10; repeat 2"]).unwrap();
    let demo = launch.demo.unwrap();
    assert_eq!(demo.steps, vec![DemoStep::Expand, DemoStep::Wait(10)]);
    assert_eq!(demo.repeat, 2);
}

#[test]
fn every_clock_name_is_recognised() {
    let names = [
        ("auto", ClockRequest::Auto),
        ("dcomp", ClockRequest::Dcomp),
        ("dxgi", ClockRequest::Dxgi),
        ("timer", ClockRequest::Timer),
    ];
    for (name, expected) in names {
        assert_eq!(parse(&["--clock", name]).unwrap().clock, expected);
    }
}

#[test]
fn unknown_flag_is_an_error() {
    assert_eq!(
        parse(&["--bogus"]),
        Err(LaunchError::UnknownFlag("--bogus".to_string()))
    );
}

#[test]
fn missing_and_bad_values_are_errors() {
    assert_eq!(
        parse(&["--demo"]),
        Err(LaunchError::MissingValue("--demo".to_string()))
    );
    assert_eq!(
        parse(&["--idle-check", "soon"]),
        Err(LaunchError::BadValue {
            flag: "--idle-check".to_string(),
            value: "soon".to_string()
        })
    );
    assert_eq!(
        parse(&["--clock", "gps"]),
        Err(LaunchError::BadValue {
            flag: "--clock".to_string(),
            value: "gps".to_string()
        })
    );
    assert!(matches!(
        parse(&["--demo", "page 9"]),
        Err(LaunchError::Demo(DemoError::PageOutOfRange(_)))
    ));
}

#[test]
fn errors_display_the_offending_text() {
    let text = LaunchError::UnknownFlag("--bogus".to_string()).to_string();
    assert!(text.contains("--bogus"));
}

#[test]
fn hit_defaults_to_region() {
    assert_eq!(parse(&[]).unwrap().hit, HitMode::Region);
}

#[test]
fn hit_parses_all_modes() {
    assert_eq!(parse(&["--hit", "region"]).unwrap().hit, HitMode::Region);
    assert_eq!(parse(&["--hit", "split"]).unwrap().hit, HitMode::Split);
    assert_eq!(parse(&["--hit", "mask"]).unwrap().hit, HitMode::Mask);
}

#[test]
fn hit_rejects_unknown_and_missing_values() {
    assert!(matches!(
        parse(&["--hit", "wide"]),
        Err(LaunchError::BadValue { .. })
    ));
    assert!(matches!(
        parse(&["--hit"]),
        Err(LaunchError::MissingValue(_))
    ));
}
