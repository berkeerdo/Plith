//! Unit tests for the pure Voicemeeter layer.

use super::*;
use std::time::{Duration, Instant};

#[test]
fn kind_counts() {
    assert_eq!(VmKind::from_api(0), None);
    assert_eq!(VmKind::from_api(4), None);
    let expect = [(1, 3, 2), (2, 5, 5), (3, 8, 8)];
    for (code, strips, buses) in expect {
        let kind = VmKind::from_api(code).unwrap();
        assert_eq!(kind.strip_count(), strips);
        assert_eq!(kind.bus_count(), buses);
    }
}

#[test]
fn clamp_gain_range_and_nan() {
    assert!((clamp_gain(-100.0) - MIN_GAIN_DB).abs() < f32::EPSILON);
    assert!((clamp_gain(20.0) - MAX_GAIN_DB).abs() < f32::EPSILON);
    assert!((clamp_gain(-3.5) - -3.5).abs() < f32::EPSILON);
    assert!((clamp_gain(f32::NAN) - MIN_GAIN_DB).abs() < f32::EPSILON);
}

#[test]
fn param_name_format() {
    assert_eq!(param_name(Rail::Strip, 2, "Gain"), "Strip[2].Gain");
    assert_eq!(param_name(Rail::Bus, 0, "Mute"), "Bus[0].Mute");
}

#[test]
fn fallback_labels() {
    assert_eq!(fallback_label(Rail::Strip, 0), "Strip 1");
    assert_eq!(fallback_label(Rail::Strip, 7), "Strip 8");
    assert_eq!(fallback_label(Rail::Bus, 0), "A1");
    assert_eq!(fallback_label(Rail::Bus, 4), "A5");
    assert_eq!(fallback_label(Rail::Bus, 5), "B1");
    assert_eq!(fallback_label(Rail::Bus, 7), "B3");
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn grace_blocks_early_negatives() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(0, t0);
    for i in 0..20 {
        let now = t0 + Duration::from_millis(100 * i);
        assert_eq!(s.on_dirty(-1, now), DirtyOutcome::Transient);
    }
}

#[test]
fn five_negatives_after_grace_is_gone() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(0, t0);
    let late = t0 + secs(6);
    for _ in 0..4 {
        assert_eq!(s.on_dirty(-1, late), DirtyOutcome::Transient);
    }
    assert_eq!(s.on_dirty(-1, late), DirtyOutcome::EngineGone);
}

#[test]
fn success_resets_streak() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(0, t0);
    let late = t0 + secs(6);
    for _ in 0..4 {
        s.on_dirty(-1, late);
    }
    assert_eq!(s.on_dirty(0, late), DirtyOutcome::Clean);
    assert_eq!(s.on_dirty(1, late), DirtyOutcome::Dirty);
    for _ in 0..4 {
        assert_eq!(s.on_dirty(-1, late), DirtyOutcome::Transient);
    }
    assert_eq!(s.on_dirty(-1, late), DirtyOutcome::EngineGone);
}

#[test]
fn rc1_uses_long_grace() {
    let t0 = Instant::now();
    let mut s = LoginSession::on_login(1, t0);
    for _ in 0..10 {
        assert_eq!(s.on_dirty(-1, t0 + secs(20)), DirtyOutcome::Transient);
    }
    let after = t0 + secs(31);
    for _ in 0..4 {
        assert_eq!(s.on_dirty(-1, after), DirtyOutcome::Transient);
    }
    assert_eq!(s.on_dirty(-1, after), DirtyOutcome::EngineGone);
}

#[test]
fn level_to_db_floor() {
    assert!((level_to_db(0.0) - LEVEL_FLOOR_DB).abs() < f32::EPSILON);
    assert!(level_to_db(1.0).abs() < 1e-4);
    assert!((level_to_db(0.5) - -6.0206).abs() < 1e-3);
    assert!((level_to_db(1e-9) - LEVEL_FLOOR_DB).abs() < f32::EPSILON);
    assert!((level_to_db(f32::NAN) - LEVEL_FLOOR_DB).abs() < f32::EPSILON);
}

#[test]
fn uninstall_string_to_directory() {
    let dir = r"C:\Program Files (x86)\VB\Voicemeeter";
    let quoted = r#""C:\Program Files (x86)\VB\Voicemeeter\voicemeeterprosetup.exe""#;
    assert_eq!(install_dir_from_uninstall(quoted), Some(dir.to_owned()));
    let bare = r"C:\Program Files (x86)\VB\Voicemeeter\voicemeeterprosetup.exe";
    assert_eq!(install_dir_from_uninstall(bare), Some(dir.to_owned()));
    let quoted_args = r#""C:\Program Files (x86)\VB\Voicemeeter\setup.exe" /u"#;
    assert_eq!(
        install_dir_from_uninstall(quoted_args),
        Some(dir.to_owned())
    );
    let bare_args = r"C:\Program Files (x86)\VB\Voicemeeter\setup.exe /u";
    assert_eq!(install_dir_from_uninstall(bare_args), Some(dir.to_owned()));
    assert_eq!(install_dir_from_uninstall(""), None);
    assert_eq!(install_dir_from_uninstall("   "), None);
    assert_eq!(install_dir_from_uninstall("setup.exe"), None);
}

#[test]
fn label_decode_stops_at_nul() {
    assert_eq!(decode_label(b"Mic\0garbage"), "Mic");
    assert_eq!(decode_label(b"\0"), "");
    assert_eq!(decode_label(b""), "");
    assert_eq!(decode_label(b"No terminator"), "No terminator");
}

#[test]
fn label_decode_utf8_then_latin1() {
    assert_eq!(
        decode_label("Kulakl\u{131}k\0".as_bytes()),
        "Kulakl\u{131}k"
    );
    // 0xE9 alone is invalid UTF-8, so it is read as Latin-1 (e with acute).
    assert_eq!(decode_label(&[b'C', b'a', b'f', 0xE9, 0]), "Caf\u{e9}");
}

#[test]
fn level_channels_per_kind() {
    let strips = |k: VmKind| -> Vec<Option<usize>> {
        (0..=k.strip_count())
            .map(|i| level_channel(k, Rail::Strip, i))
            .collect()
    };
    assert_eq!(strips(VmKind::Standard), [Some(0), Some(2), Some(4), None]);
    let banana = [Some(0), Some(2), Some(4), Some(6), Some(14), None];
    assert_eq!(strips(VmKind::Banana), banana);
    let potato = [
        Some(0),
        Some(2),
        Some(4),
        Some(6),
        Some(8),
        Some(10),
        Some(18),
        Some(26),
        None,
    ];
    assert_eq!(strips(VmKind::Potato), potato);
}

#[test]
fn level_channels_for_buses() {
    assert_eq!(level_channel(VmKind::Standard, Rail::Bus, 1), Some(8));
    assert_eq!(level_channel(VmKind::Standard, Rail::Bus, 2), None);
    assert_eq!(level_channel(VmKind::Banana, Rail::Bus, 4), Some(32));
    assert_eq!(level_channel(VmKind::Banana, Rail::Bus, 5), None);
    assert_eq!(level_channel(VmKind::Potato, Rail::Bus, 7), Some(56));
}

#[test]
fn level_type_per_rail() {
    assert_eq!(level_type(Rail::Strip), 1);
    assert_eq!(level_type(Rail::Bus), 3);
}
