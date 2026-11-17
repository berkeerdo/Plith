use super::*;
use crate::demo::{DemoScript, DemoStep};
use crate::levers::{Levers, MmcssPriority, SchedClass};
use crate::tuning::{BRISK, R1};

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
fn hit_flag_is_unknown() {
    assert_eq!(
        parse(&["--hit", "region"]),
        Err(LaunchError::UnknownFlag("--hit".to_string()))
    );
    assert!(matches!(
        parse(&["--hit"]),
        Err(LaunchError::UnknownFlag(_))
    ));
}

#[test]
fn parses_every_lever_flag() {
    let launch = parse(&[
        "--gpu-priority",
        "-7",
        "--sched-class",
        "above",
        "--latency",
        "3",
        "--sync",
        "0",
        "--mmcss",
        "critical",
    ])
    .unwrap();
    assert_eq!(launch.levers.gpu_priority, -7);
    assert_eq!(launch.levers.sched_class, SchedClass::AboveNormal);
    assert_eq!(launch.levers.latency, 3);
    assert_eq!(launch.levers.sync, 0);
    assert_eq!(launch.levers.mmcss, MmcssPriority::Critical);
    assert_eq!(parse(&[]).unwrap().levers, Levers::default());
    assert_eq!(
        parse(&["--sched-class", "high", "--mmcss", "high"])
            .unwrap()
            .levers
            .sched_class,
        SchedClass::High
    );
}

#[test]
fn rejects_gpu_priority_8_and_latency_0() {
    for (flag, value) in [
        ("--gpu-priority", "8"),
        ("--gpu-priority", "-8"),
        ("--latency", "0"),
        ("--latency", "4"),
        ("--sync", "2"),
        ("--sched-class", "realtime"),
        ("--mmcss", "low"),
    ] {
        assert_eq!(
            parse(&[flag, value]),
            Err(LaunchError::BadValue {
                flag: flag.to_string(),
                value: value.to_string()
            }),
            "{flag} {value}"
        );
    }
}

#[test]
fn tune_flag_picks_a_preset_and_defaults_to_brisk() {
    assert_eq!(parse(&[]).unwrap().tune, BRISK);
    assert_eq!(parse(&["--tune", "r1"]).unwrap().tune, R1);
    assert_eq!(parse(&["--tune", "brisk"]).unwrap().tune, BRISK);
    assert!(matches!(
        parse(&["--tune", "fast"]),
        Err(LaunchError::BadValue { .. })
    ));
    assert!(matches!(
        parse(&["--tune"]),
        Err(LaunchError::MissingValue(_))
    ));
}

#[test]
fn idle_check_over_70_is_rejected() {
    assert_eq!(
        parse(&["--idle-check", "70"]).unwrap().idle_check_secs,
        Some(70)
    );
    assert_eq!(
        parse(&["--idle-check", "71"]),
        Err(LaunchError::BadValue {
            flag: "--idle-check".to_string(),
            value: "71".to_string()
        })
    );
}

#[test]
fn fixture_flag_names_the_mock_data() {
    assert_eq!(parse(&[]).unwrap().fixture, None);
    let pick = |name: &str| parse(&["--fixture", name]).unwrap().fixture;
    assert_eq!(pick("banana"), Some(Fixture::Banana));
    assert_eq!(pick("potato"), Some(Fixture::Potato));
    assert_eq!(pick("bare"), Some(Fixture::Bare));
    assert_eq!(pick("none"), None);
}

#[test]
fn bad_or_missing_fixture_is_rejected() {
    assert_eq!(
        parse(&["--fixture", "standard"]),
        Err(LaunchError::BadValue {
            flag: "--fixture".to_string(),
            value: "standard".to_string(),
        })
    );
    assert_eq!(
        parse(&["--fixture"]),
        Err(LaunchError::MissingValue("--fixture".to_string()))
    );
}

#[test]
fn flyout_modes_parse_and_default_to_off() {
    assert_eq!(parse(&[]).unwrap().flyout, FlyoutMode::Off);
    let names = [
        ("off", FlyoutMode::Off),
        ("dry", FlyoutMode::DryRun),
        ("hide", FlyoutMode::Hide),
    ];
    for (name, expected) in names {
        assert_eq!(parse(&["--flyout", name]).unwrap().flyout, expected);
    }
}

#[test]
fn bad_or_missing_flyout_is_rejected() {
    assert_eq!(
        parse(&["--flyout", "x"]),
        Err(LaunchError::BadValue {
            flag: "--flyout".to_string(),
            value: "x".to_string(),
        })
    );
    assert_eq!(
        parse(&["--flyout"]),
        Err(LaunchError::MissingValue("--flyout".to_string()))
    );
}

#[test]
fn idle_look_takes_a_look_key() {
    assert_eq!(parse(&[]).unwrap().idle_look, None);
    for look in [
        IdleLook::Karot,
        IdleLook::Clock,
        IdleLook::Playing,
        IdleLook::Seal,
        IdleLook::Plain,
        IdleLook::Hidden,
    ] {
        let launch = parse(&["--idle-look", look.key()]).unwrap();
        assert_eq!(launch.idle_look, Some(look));
    }
    assert!(matches!(
        parse(&["--idle-look", "neon"]),
        Err(LaunchError::BadValue { .. })
    ));
    assert!(matches!(
        parse(&["--idle-look"]),
        Err(LaunchError::MissingValue(_))
    ));
}
