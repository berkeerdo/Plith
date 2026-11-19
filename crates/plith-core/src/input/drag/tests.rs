use super::*;

fn ruler() -> Rect {
    Rect {
        x: 100.0,
        y: 50.0,
        w: 200.0,
        h: 8.0,
    }
}

fn target(action: Action) -> HitTarget {
    HitTarget {
        rect: ruler(),
        action,
    }
}

#[test]
fn begin_on_volume_ruler() {
    let targets = [target(Action::VolumeRuler(ruler()))];
    let drag = begin_drag(&targets, 150.0, 54.0).expect("on the ruler");
    assert_eq!(drag.kind, RulerKind::Volume);
    assert_eq!(drag.ruler, ruler());
    assert_eq!(drag.value_at(200.0), Action::SetVolume(50));
}

#[test]
fn begin_on_brightness_ruler() {
    let targets = [target(Action::BrightnessRuler(ruler()))];
    let drag = begin_drag(&targets, 150.0, 54.0).expect("on the ruler");
    assert_eq!(drag.kind, RulerKind::Brightness);
    assert_eq!(drag.value_at(250.0), Action::SetBrightness(75));
}

#[test]
fn begin_on_seek_ruler() {
    let targets = [target(Action::SeekRuler(ruler()))];
    let drag = begin_drag(&targets, 150.0, 54.0).expect("on the ruler");
    assert_eq!(drag.kind, RulerKind::Seek);
    assert_eq!(drag.value_at(150.0), Action::Seek(0.25));
}

#[test]
fn begin_uses_hit_slack() {
    let targets = [target(Action::VolumeRuler(ruler()))];
    assert!(begin_drag(&targets, 96.0, 54.0).is_some());
    assert!(begin_drag(&targets, 94.0, 54.0).is_none());
}

#[test]
fn begin_off_ruler_is_none() {
    let tab = target(Action::Tab(crate::pages::PageId::Media));
    assert!(begin_drag(&[tab], 150.0, 54.0).is_none());
    assert!(begin_drag(&[], 150.0, 54.0).is_none());
}

#[test]
fn value_clamps_past_both_ends() {
    let volume = RulerDrag {
        kind: RulerKind::Volume,
        ruler: ruler(),
    };
    assert_eq!(volume.value_at(-500.0), Action::SetVolume(0));
    assert_eq!(volume.value_at(900.0), Action::SetVolume(100));
    let seek = RulerDrag {
        kind: RulerKind::Seek,
        ruler: ruler(),
    };
    assert_eq!(seek.value_at(-500.0), Action::Seek(0.0));
    assert_eq!(seek.value_at(900.0), Action::Seek(1.0));
}

#[test]
fn throttle_lets_one_per_gap() {
    let mut throttle = Throttle::new(DRAG_WRITE_GAP_S);
    assert!(throttle.due(0.0));
    assert!(!throttle.due(0.02));
    assert!(throttle.due(0.034));
    assert!(!throttle.due(0.05));
}
