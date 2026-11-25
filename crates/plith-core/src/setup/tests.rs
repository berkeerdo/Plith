use super::*;

fn v(major: u32, minor: u32, patch: u32) -> Version {
    Version::new(major, minor, patch)
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_string()).collect()
}

const PIPE: &str = "\\\\.\\pipe\\plith-setup-abc";

#[test]
fn version_orders_and_parses() {
    let old = Version::parse("0.3.3+5e2feb9").unwrap();
    assert_eq!(old, v(0, 3, 3));
    assert!(old < v(0, 4, 0));
    assert!(v(0, 4, 0) < v(0, 10, 0));
    assert_eq!(Version::parse("v0.4.0"), Some(v(0, 4, 0)));
    assert_eq!(v(0, 4, 0).to_string(), "0.4.0");
    assert_eq!(Version::parse("0.4"), None);
    assert_eq!(Version::parse("abc"), None);
    assert_eq!(Version::parse("0.4.0.1"), None);
    assert_eq!(Version::parse("0.+4.0"), None);
}

#[test]
fn situation_over_wpf_on_this_pc() {
    assert_eq!(
        situation(v(0, 4, 0), Some("0.3.3+5e2feb9"), true),
        Situation::OverWpf { from: v(0, 3, 3) }
    );
    assert_eq!(
        situation(v(0, 4, 0), Some("0.3.1"), false),
        Situation::OverWpf { from: v(0, 3, 1) }
    );
    assert_eq!(
        situation(v(0, 4, 0), None, true),
        Situation::OverWpf { from: v(0, 3, 0) }
    );
}

#[test]
fn situation_newer_installed() {
    assert_eq!(
        situation(v(0, 4, 0), Some("0.5.0"), false),
        Situation::NewerInstalled {
            installed: v(0, 5, 0)
        }
    );
}

#[test]
fn situation_upgrade_from_older_rust_build() {
    assert_eq!(
        situation(v(0, 4, 2), Some("0.4.0"), false),
        Situation::Upgrade { from: v(0, 4, 0) }
    );
}

#[test]
fn situation_repair_on_equal() {
    assert_eq!(
        situation(v(0, 4, 0), Some("0.4.0"), false),
        Situation::Repair
    );
}

#[test]
fn situation_repair_on_broken_entry() {
    assert_eq!(
        situation(v(0, 4, 0), Some("garbage"), false),
        Situation::Repair
    );
}

#[test]
fn situation_fresh_without_entry() {
    assert_eq!(situation(v(0, 4, 0), None, false), Situation::Fresh);
}

#[test]
fn is_ours_keeps_only_three_files() {
    assert!(is_ours("plith.exe"));
    assert!(is_ours("Plith.EXE"));
    assert!(is_ours("osd\\plith-osd.exe"));
    assert!(is_ours("osd/plith-osd.exe"));
    assert!(is_ours("setup\\plith-setup.exe"));
    assert!(!is_ours("Plith.dll"));
    assert!(!is_ours("runtimes\\win\\x.dll"));
    assert!(!is_ours("Setup\\Plith-Uninstaller.exe"));
}

#[test]
fn wire_round_trips() {
    for report in [
        Report::Step(1),
        Report::Step(3),
        Report::Log("copying plith.exe".to_string()),
        Report::Log(String::new()),
        Report::Failed {
            step: 2,
            reason: "access: denied".to_string(),
        },
        Report::Finished,
    ] {
        assert_eq!(decode(&encode(&report)), Some(report));
    }
}

#[test]
fn wire_caps_long_text_on_a_char_boundary() {
    let long = "\u{e9}".repeat(150);
    let Some(Report::Log(text)) = decode(&encode(&Report::Log(long))) else {
        panic!("log did not round-trip");
    };
    assert_eq!(text.len(), 200);
    let Some(Report::Failed { reason, .. }) = decode(&encode(&Report::Failed {
        step: 1,
        reason: "x".repeat(500),
    })) else {
        panic!("failure did not round-trip");
    };
    assert_eq!(reason.len(), 200);
}

#[test]
fn wire_rejects_garbage_and_step_out_of_range() {
    for bad in [
        &b""[..],
        b"X",
        b"S",
        b"S0",
        b"S4",
        b"Sx",
        b"F0:x",
        b"F4:x",
        b"F2",
        b"Dx",
        b"\xff\xfe",
    ] {
        assert_eq!(decode(bad), None, "{bad:?}");
    }
    assert_eq!(decode(&[b'L'; 202]), None);
}

#[test]
fn args_work_needs_job_and_pipe() {
    assert_eq!(
        parse_args(&strings(&["--work", "install", PIPE])),
        Mode::Work {
            job: Job::Install,
            pipe: PIPE.to_string()
        }
    );
    assert_eq!(
        parse_args(&strings(&["--work", "uninstall", PIPE])),
        Mode::Work {
            job: Job::Uninstall,
            pipe: PIPE.to_string()
        }
    );
    assert_eq!(parse_args(&strings(&["--work", "install"])), Mode::Install);
    assert_eq!(
        parse_args(&strings(&["--work", "explode", PIPE])),
        Mode::Install
    );
    assert_eq!(
        parse_args(&strings(&["--work", "install", "\\\\.\\pipe\\other"])),
        Mode::Install
    );
}

#[test]
fn args_other_modes() {
    assert_eq!(parse_args(&[]), Mode::Install);
    assert_eq!(parse_args(&strings(&["--bogus"])), Mode::Install);
    assert_eq!(parse_args(&strings(&["--uninstall"])), Mode::Uninstall);
    assert_eq!(
        parse_args(&strings(&["--update", "--from", "0.3.3+abc"])),
        Mode::Update { from: v(0, 3, 3) }
    );
    assert_eq!(
        parse_args(&strings(&["--update", "--from", "zzz"])),
        Mode::Install
    );
    assert_eq!(parse_args(&strings(&["--update"])), Mode::Install);
}

#[test]
fn quote_arg_handles_spaces_parens_and_quotes() {
    assert_eq!(quote_arg("plain"), "plain");
    assert_eq!(quote_arg(""), "\"\"");
    assert_eq!(quote_arg("a b (1)"), "\"a b (1)\"");
    assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
    assert_eq!(quote_arg("C:\\x y\\"), "\"C:\\x y\\\\\"");
}
