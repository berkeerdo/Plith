use super::*;
use crate::pages::hit;

fn stations(nodes: &[Node]) -> Vec<(f64, StationMark, Ink)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Station { x, mark, ink, .. } => Some((*x, *mark, *ink)),
            _ => None,
        })
        .collect()
}

#[test]
fn four_stations_one_line_between_ends() {
    let (nodes, _) = tabbar(PageId::Audio, "21:47");
    let xs: Vec<f64> = stations(&nodes).iter().map(|s| s.0).collect();
    assert_eq!(xs, vec![30.0, 142.0, 254.0, 366.0]);
    let lines: Vec<_> = nodes
        .iter()
        .filter(|n| matches!(n, Node::Stroke { .. }))
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        &Node::Stroke {
            points: vec![(30.0, 34.0), (366.0, 34.0)],
            width: 1.0,
            ink: Ink::Track,
            dashed: false,
        }
    );
}

#[test]
fn active_is_filled_ink() {
    let (nodes, _) = tabbar(PageId::Shelf, "21:47");
    let marks = stations(&nodes);
    for (i, (_, mark, ink)) in marks.iter().enumerate() {
        if i == 2 {
            assert_eq!((*mark, *ink), (StationMark::Filled, Ink::Ink));
        } else {
            assert_eq!((*mark, *ink), (StationMark::Hollow, Ink::Faint));
        }
    }
    let labels: Vec<(String, Ink)> = nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text {
                text,
                ink,
                style: TextStyle::Label,
                ..
            } => Some((text.to_string(), *ink)),
            _ => None,
        })
        .collect();
    assert_eq!(
        labels,
        vec![
            ("SES".to_string(), Ink::Faint),
            ("MEDYA".to_string(), Ink::Faint),
            ("RAF".to_string(), Ink::Ink),
            ("S\u{130}STEM".to_string(), Ink::Faint),
        ]
    );
}

#[test]
fn tab_hit_targets_cover_labels() {
    let (_, targets) = tabbar(PageId::Audio, "21:47");
    assert_eq!(targets.len(), 4);
    assert_eq!(hit(&targets, 40.0, 20.0), Some(Action::Tab(PageId::Audio)));
    assert_eq!(hit(&targets, 150.0, 38.0), Some(Action::Tab(PageId::Media)));
    assert_eq!(hit(&targets, 262.0, 20.0), Some(Action::Tab(PageId::Shelf)));
    assert_eq!(
        hit(&targets, 440.0, 30.0),
        Some(Action::Tab(PageId::System))
    );
    assert_eq!(hit(&targets, 300.0, 100.0), None);
}

#[test]
fn clock_pads_zero() {
    assert_eq!(clock_text(9, 5), "09:05");
    assert_eq!(clock_text(21, 47), "21:47");
}

#[test]
fn clock_is_right_aligned_at_the_margin() {
    let (nodes, _) = tabbar(PageId::Audio, "21:47");
    let clock = nodes
        .iter()
        .find_map(|n| match n {
            Node::Text {
                rect,
                style: TextStyle::Clock,
                align,
                ..
            } => Some((*rect, *align)),
            _ => None,
        })
        .expect("clock node");
    assert!((clock.0.x + clock.0.w - 596.0).abs() < 1e-9);
    assert!((clock.0.y + clock.0.h / 2.0 - 34.0).abs() < 1e-9);
    assert_eq!(clock.1, Align::End);
}

#[test]
fn page_labels_and_indices() {
    assert_eq!(PageId::System.label(), "S\u{130}STEM");
    for (i, page) in PageId::ALL.into_iter().enumerate() {
        assert_eq!(page.index(), i);
        assert_eq!(PageId::from_index(i), Some(page));
    }
    assert_eq!(PageId::from_index(4), None);
}

#[test]
fn hit_slack_four_dip() {
    let targets = vec![HitTarget {
        rect: Rect {
            x: 10.0,
            y: 10.0,
            w: 20.0,
            h: 20.0,
        },
        action: Action::Tab(PageId::Media),
    }];
    assert!(hit(&targets, 6.0, 20.0).is_some());
    assert!(hit(&targets, 34.0, 34.0).is_some());
    assert!(hit(&targets, 5.9, 20.0).is_none());
    assert!(hit(&targets, 20.0, 34.1).is_none());
}
