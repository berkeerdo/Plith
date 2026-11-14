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
fn fallback_labels_strips() {
    assert_eq!(fallback_label(VmKind::Banana, Rail::Strip, 0), "Strip 1");
    assert_eq!(fallback_label(VmKind::Potato, Rail::Strip, 7), "Strip 8");
}

fn bus_labels(kind: VmKind) -> Vec<String> {
    (0..kind.bus_count())
        .map(|i| fallback_label(kind, Rail::Bus, i))
        .collect()
}

#[test]
fn fallback_labels_buses_per_kind() {
    assert_eq!(bus_labels(VmKind::Standard), ["A1", "A2"]);
    assert_eq!(bus_labels(VmKind::Banana), ["A1", "A2", "A3", "B1", "B2"]);
    let potato = ["A1", "A2", "A3", "A4", "A5", "B1", "B2", "B3"];
    assert_eq!(bus_labels(VmKind::Potato), potato);
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn label_decode_stops_at_nul() {
    assert_eq!(decode_label(&wide("Mic\0garbage")), "Mic");
    assert_eq!(decode_label(&[0]), "");
    assert_eq!(decode_label(&[]), "");
    assert_eq!(decode_label(&wide("No terminator")), "No terminator");
}

#[test]
fn label_decode_keeps_turkish_and_replaces_bad_surrogates() {
    assert_eq!(
        decode_label(&wide("Kulakl\u{131}k \u{15f}\u{11f}\0")),
        "Kulakl\u{131}k \u{15f}\u{11f}"
    );
    assert_eq!(decode_label(&[0x41, 0xD800, 0x42, 0]), "A\u{fffd}B");
}

#[test]
fn candidate_paths_are_absolute_only() {
    let dll = "VoicemeeterRemote64.dll";
    let default = r"C:\Program Files (x86)\VB\Voicemeeter";
    let dirs = vec![
        r"D:\Audio\VB".to_owned(),
        "relative\\dir".to_owned(),
        r"\\server\share\VB".to_owned(),
        r"D:\Audio\VB".to_owned(),
    ];
    assert_eq!(
        candidate_paths(&dirs, default, dll),
        [
            r"D:\Audio\VB\VoicemeeterRemote64.dll",
            r"\\server\share\VB\VoicemeeterRemote64.dll",
            r"C:\Program Files (x86)\VB\Voicemeeter\VoicemeeterRemote64.dll",
        ]
    );
    // Never a bare name, even with nothing registered.
    assert_eq!(candidate_paths(&[], default, dll).len(), 1);
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
