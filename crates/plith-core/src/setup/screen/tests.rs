use super::*;
use crate::lang::setup::setup_text;

fn own() -> Version {
    Version::new(0, 4, 0)
}

fn build(screen: &Screen) -> (DisplayList, Vec<SetupTarget>) {
    setup_build(screen, own(), Lang::Tr)
}

fn texts(list: &DisplayList) -> Vec<String> {
    list.nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

fn background(list: &DisplayList) -> ArtId {
    match list.nodes.first() {
        Some(Node::Image { rect, art }) => {
            assert_eq!((rect.w, rect.h), (SETUP_W, SETUP_H));
            *art
        }
        other => panic!("first node is not the background: {other:?}"),
    }
}

fn action_rect(targets: &[SetupTarget], action: SetupAction) -> Option<Rect> {
    targets.iter().find(|t| t.action == action).map(|t| t.rect)
}

fn tr(id: SetupText) -> &'static str {
    setup_text(id, Lang::Tr)
}

#[test]
fn start_has_button_at_readme_rect() {
    let (list, targets) = build(&Screen::Start);
    assert_eq!(list.size, (SETUP_W, SETUP_H));
    assert_eq!(background(&list), BG_ART[0]);
    let r = action_rect(&targets, SetupAction::Install).unwrap();
    assert_eq!((r.x, r.y, r.w, r.h), (220.0, 298.0, 200.0, 55.0));
    assert!(list.nodes.contains(&Node::Image {
        rect: r,
        art: BUTTON_ART
    }));
    let lime_label = list.nodes.iter().any(|n| {
        matches!(n, Node::Text { text, ink: Ink::Flow, .. } if &**text == tr(SetupText::Install))
    });
    assert!(lime_label, "the button label is the lime flow ink");
    assert!(texts(&list).contains(&"0.4.0".to_owned()));
    assert!(texts(&list).contains(&tr(SetupText::LeadStart).to_owned()));
    assert_eq!(
        setup_hit(&targets, 320.0, 320.0),
        Some(SetupAction::Install)
    );
}

#[test]
fn running_draws_background_for_done_steps() {
    for done in 0..=3u8 {
        let (list, targets) = build(&Screen::Running { done });
        assert_eq!(background(&list), BG_ART[usize::from(done)]);
        assert!(action_rect(&targets, SetupAction::Close).is_none());
        let t = texts(&list);
        assert!(t.contains(&format!("1 {}", tr(SetupText::Step1))));
        assert!(t.contains(&format!("3 {}", tr(SetupText::Step3))));
        assert!(t.contains(&tr(SetupText::LeadRunning).to_owned()));
        assert_eq!(
            t.contains(&tr(SetupText::SubUac).to_owned()),
            done < 2,
            "the UAC line is named before the prompt"
        );
    }
}

#[test]
fn done_targets_open_autostart_close() {
    let (list, targets) = build(&Screen::Done { autostart: false });
    assert_eq!(background(&list), BG_ART[3]);
    let open = action_rect(&targets, SetupAction::Open).unwrap();
    assert_eq!((open.x, open.y), (220.0, 304.0));
    let auto = action_rect(&targets, SetupAction::ToggleAutostart).unwrap();
    assert!(auto.y < 370.0 && auto.y + auto.h > 370.0);
    let close = targets
        .iter()
        .filter(|t| t.action == SetupAction::Close)
        .map(|t| t.rect)
        .collect::<Vec<_>>();
    assert_eq!(close.len(), 2, "the caption and the close mark");
    assert!(close.iter().any(|r| r.y < 394.0 && r.y + r.h > 394.0));
    assert_eq!(setup_hit(&targets, 320.0, 394.0), Some(SetupAction::Close));
    assert_eq!(setup_hit(&targets, 624.0, 20.0), Some(SetupAction::Close));
    let boxes = |on: bool| {
        let (l, _) = build(&Screen::Done { autostart: on });
        l.nodes
            .iter()
            .filter(|n| matches!(n, Node::Fill { .. }))
            .count()
    };
    assert_eq!(boxes(true), boxes(false) + 1, "a ticked box has its mark");
}

#[test]
fn error_names_step_and_retry() {
    let screen = Screen::Error {
        step: 2,
        reason: "disk full".to_owned(),
    };
    let (list, targets) = build(&screen);
    assert_eq!(background(&list), BG_ART[1]);
    let t = texts(&list);
    assert!(t.contains(&format!("2 {}", tr(SetupText::Step2))));
    assert!(!t.contains(&format!("1 {}", tr(SetupText::Step1))));
    assert!(t.contains(&"disk full".to_owned()));
    assert!(t.contains(&tr(SetupText::ErrorTitle).to_owned()));
    assert!(action_rect(&targets, SetupAction::Retry).is_some());
    assert!(action_rect(&targets, SetupAction::Close).is_some());
}

#[test]
fn uninstall_backgrounds_run_in_reverse() {
    for done in 0..=3u8 {
        let (list, targets) = build(&Screen::Uninstalling { done });
        assert_eq!(background(&list), BG_ART[usize::from(3 - done)]);
        assert!(action_rect(&targets, SetupAction::Close).is_none());
    }
    let (list, _) = build(&Screen::UninstallConfirm { delete_data: true });
    assert_eq!(background(&list), BG_ART[3]);
    let (list, _) = build(&Screen::Uninstalled);
    assert_eq!(background(&list), BG_ART[0]);
}

#[test]
fn drag_area_above_ripples() {
    let (_, targets) = build(&Screen::Start);
    assert_eq!(setup_hit(&targets, 320.0, 100.0), Some(SetupAction::Drag));
    assert_eq!(setup_hit(&targets, 320.0, 214.0), Some(SetupAction::Drag));
    assert_eq!(setup_hit(&targets, 100.0, 216.0), None);
    assert_eq!(targets.last().map(|t| t.action), Some(SetupAction::Drag));
    let (_, running) = build(&Screen::Running { done: 1 });
    assert_eq!(setup_hit(&running, 624.0, 20.0), Some(SetupAction::Drag));
}

#[test]
fn others_follow_start_rules() {
    let (list, targets) = build(&Screen::Update {
        from: Version::new(0, 4, 0),
        to: Version::new(0, 5, 0),
    });
    assert!(texts(&list).contains(&"0.4.0 \u{2192} 0.5.0".to_owned()));
    assert!(texts(&list).contains(&tr(SetupText::Update).to_owned()));
    assert_eq!(
        action_rect(&targets, SetupAction::Install).map(|r| (r.x, r.y)),
        Some((220.0, 298.0))
    );

    let (list, targets) = build(&Screen::Newer {
        installed: Version::new(0, 9, 0),
    });
    assert!(texts(&list).contains(&"0.9.0".to_owned()));
    assert!(action_rect(&targets, SetupAction::Install).is_none());

    let (list, targets) = build(&Screen::UninstallConfirm { delete_data: false });
    assert!(action_rect(&targets, SetupAction::Uninstall).is_some());
    assert!(action_rect(&targets, SetupAction::ToggleDeleteData).is_some());
    assert!(texts(&list).contains(&tr(SetupText::DeleteData).to_owned()));

    let (start, _) = build(&Screen::Start);
    let (cancelled, targets) = build(&Screen::UacCancelled);
    assert!(texts(&cancelled).contains(&tr(SetupText::UacCancelled).to_owned()));
    assert!(!texts(&start).contains(&tr(SetupText::UacCancelled).to_owned()));
    assert!(action_rect(&targets, SetupAction::Install).is_some());
}

#[test]
fn every_screen_in_eight_languages_builds() {
    let screens = [
        Screen::Start,
        Screen::Running { done: 1 },
        Screen::Done { autostart: true },
        Screen::Error {
            step: 3,
            reason: "x".to_owned(),
        },
        Screen::Newer {
            installed: Version::new(1, 0, 0),
        },
        Screen::Update {
            from: Version::new(0, 4, 0),
            to: Version::new(0, 5, 0),
        },
        Screen::UninstallConfirm { delete_data: true },
        Screen::Uninstalling { done: 2 },
        Screen::Uninstalled,
        Screen::UacCancelled,
    ];
    for lang in Lang::ALL {
        for screen in &screens {
            let (list, targets) = setup_build(screen, own(), lang);
            assert_eq!(list.size, (SETUP_W, SETUP_H), "{screen:?} {lang:?}");
            assert!(texts(&list).iter().all(|t| !t.is_empty()));
            assert!(
                targets
                    .iter()
                    .all(|t| t.rect.x >= 0.0 && t.rect.x + t.rect.w <= SETUP_W),
                "{screen:?} {lang:?}"
            );
        }
    }
}
