use super::*;
use crate::lang::Lang;
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
    let (nodes, _) = tabbar(Some(PageId::Audio), "21:47", Lang::Tr);
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
            ink: Ink::FaintTrack,
            dashed: false,
        }
    );
}

#[test]
fn active_is_filled_ink() {
    let (nodes, _) = tabbar(Some(PageId::Shelf), "21:47", Lang::Tr);
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
    let (_, targets) = tabbar(Some(PageId::Audio), "21:47", Lang::Tr);
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
    let (nodes, _) = tabbar(Some(PageId::Audio), "21:47", Lang::Tr);
    let clock = nodes
        .iter()
        .find_map(|n| match n {
            Node::Text {
                rect,
                style: TextStyle::Clock,
                align,
                ink,
                ..
            } => Some((*rect, *align, *ink)),
            _ => None,
        })
        .expect("clock node");
    assert!((clock.0.x + clock.0.w - 596.0).abs() < 1e-9);
    assert!((clock.0.y + clock.0.h / 2.0 - 34.0).abs() < 1e-9);
    assert_eq!(clock.1, Align::End);
    assert_eq!(clock.2, Ink::Muted);
}

#[test]
fn seam_between_two_tabs_goes_to_the_earlier() {
    let (_, targets) = tabbar(Some(PageId::Audio), "21:47", Lang::Tr);
    let first = targets[0].rect;
    let second = targets[1].rect;
    // Grown by 4 DIP each, the two rects touch exactly at this x.
    let seam = first.x + first.w + 4.0;
    assert!((seam - (second.x - 4.0)).abs() < 1e-9);
    assert_eq!(hit(&targets, seam, 20.0), Some(Action::Tab(PageId::Audio)));
}

#[test]
fn page_labels_and_indices() {
    assert_eq!(PageId::System.label(Lang::Tr), "S\u{130}STEM");
    for (i, page) in PageId::ALL.into_iter().enumerate() {
        assert_eq!(page.index(), i);
        assert_eq!(PageId::from_index(i), Some(page));
    }
    assert_eq!(PageId::from_index(5), None);
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

#[test]
fn no_active_page_fills_no_station() {
    let (nodes, targets) = tabbar(None, "", Lang::Tr);
    assert!(
        stations(&nodes)
            .iter()
            .all(|s| s.1 == StationMark::Hollow && s.2 == Ink::Faint)
    );
    assert_eq!(targets.len(), 4);
}

#[test]
fn empty_clock_draws_no_clock_text() {
    let (nodes, _) = tabbar(None, "", Lang::Tr);
    let clocks = nodes
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::Text {
                    style: TextStyle::Clock,
                    ..
                }
            )
        })
        .count();
    assert_eq!(clocks, 0);
}

#[test]
fn tabbar_on_welcome_has_no_filled_station() {
    let (nodes, targets) = tabbar(Some(PageId::Welcome), "21:47", Lang::Tr);
    let marks = stations(&nodes);
    assert_eq!(marks.len(), 4);
    assert!(
        marks
            .iter()
            .all(|s| s.1 == StationMark::Hollow && s.2 == Ink::Faint)
    );
    assert_eq!(targets.len(), 4);
    assert_eq!(PageId::TABS.len(), 4);
    assert!(!PageId::TABS.contains(&PageId::Welcome));
    assert_eq!(PageId::ALL.len(), crate::pages::PAGE_COUNT);
}

fn tab_labels(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text {
                text,
                style: TextStyle::Label,
                ..
            } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn tabs_follow_the_language() {
    let (turkish, _) = tabbar(Some(PageId::Audio), "21:47", Lang::Tr);
    assert_eq!(
        tab_labels(&turkish),
        ["SES", "MEDYA", "RAF", "S\u{130}STEM"]
    );
    let (german, _) = tabbar(Some(PageId::Audio), "21:47", Lang::De);
    assert_eq!(tab_labels(&german), ["AUDIO", "MEDIEN", "ABLAGE", "SYSTEM"]);
    assert_eq!(PageId::Media.label(Lang::De), "MEDIEN");
    assert_eq!(PageId::Welcome.label(Lang::De), "");
}
