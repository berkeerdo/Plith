use super::*;
use crate::draw::{Ink, Node, StationMark};
use crate::pages::{Action, hit};

fn facts(look: IdleLook) -> QuickFacts {
    QuickFacts {
        look,
        previews: std::array::from_fn(|_| {
            Arc::new(DisplayList {
                nodes: Vec::new(),
                size: (200.0, 32.0),
            })
        }),
        lang: Lang::Tr,
    }
}

fn looks(build: &PageBuild) -> Vec<(IdleLook, Rect)> {
    build
        .targets
        .iter()
        .filter_map(|t| match t.action {
            Action::Look(look) => Some((look, t.rect)),
            _ => None,
        })
        .collect()
}

#[test]
fn quick_has_six_looks() {
    let build = quick_page(&facts(IdleLook::Karot));
    let found = looks(&build);
    let order: Vec<IdleLook> = found.iter().map(|l| l.0).collect();
    assert_eq!(
        order,
        [
            IdleLook::Karot,
            IdleLook::Clock,
            IdleLook::Playing,
            IdleLook::Seal,
            IdleLook::Plain,
            IdleLook::Hidden
        ]
    );
    let insets: Vec<f64> = build
        .list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Inset { scale, .. } => Some(*scale),
            _ => None,
        })
        .collect();
    assert_eq!(insets, vec![0.4; 6]);
    // One row across 560 DIP, left to right, inside the margins.
    assert!(found[0].1.x >= 30.0 && found[5].1.x + found[5].1.w <= 590.0 + 1e-9);
    assert!(found.windows(2).all(|w| w[0].1.x + w[0].1.w <= w[1].1.x));
    assert!(found.windows(2).all(|w| (w[0].1.y - w[1].1.y).abs() < 1e-9));
}

#[test]
fn quick_marks_the_picked_look() {
    let build = quick_page(&facts(IdleLook::Seal));
    let marks: Vec<StationMark> = build
        .list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Station { mark, y, .. } if *y > 60.0 => Some(*mark),
            _ => None,
        })
        .collect();
    assert_eq!(marks.len(), 6);
    assert_eq!(
        marks.iter().filter(|m| **m == StationMark::Filled).count(),
        1
    );
    assert_eq!(marks[3], StationMark::Filled);
}

#[test]
fn quick_open_settings_row() {
    let build = quick_page(&facts(IdleLook::Karot));
    let row = build
        .targets
        .iter()
        .find(|t| t.action == Action::OpenSettings)
        .expect("open settings target");
    let (x, y) = (row.rect.x + row.rect.w / 2.0, row.rect.y + row.rect.h / 2.0);
    assert_eq!(hit(&build.targets, x, y), Some(Action::OpenSettings));
    assert!(row.rect.w >= 560.0 - 1e-9);
    let words: Vec<String> = build
        .list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect();
    assert!(words.contains(&"KAPALI ADA".to_string()));
    assert!(words.contains(&"T\u{dc}M AYARLAR".to_string()));
    assert_eq!(build.shape, crate::shapes::QUICK);
    assert!((build.list.size.1 - build.shape.height).abs() < 1e-9);
}

#[test]
fn quick_gear_is_inked_and_goes_home() {
    let build = quick_page(&facts(IdleLook::Karot));
    let last = build
        .targets
        .iter()
        .find(|t| t.rect.x > 570.0 && t.rect.y < 40.0);
    assert_eq!(
        last.map(|t| t.action.clone()),
        Some(Action::Tab(PageId::Welcome))
    );
    assert!(build.list.nodes.iter().any(|n| matches!(
        n,
        Node::Stroke { ink: Ink::Ink, width, .. } if *width > 1.0
    )));
}

#[test]
fn looks_follow_idle_look_all() {
    let order: Vec<IdleLook> = LOOKS.iter().map(|l| l.0).collect();
    assert_eq!(order, IdleLook::ALL.to_vec());
}
