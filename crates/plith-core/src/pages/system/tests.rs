#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Align, Ink, Node, StationMark, TextStyle};
use crate::pages::{Action, hit};

fn facts() -> SystemFacts {
    SystemFacts {
        brightness: Some(("DELL AW2725DF".into(), 70)),
        mic: Some(MicFacts {
            name: "HW 1 · SHURE MV7".into(),
            muted: false,
        }),
        mic_bus: Some("B1".into()),
    }
}

fn muted() -> SystemFacts {
    let mut f = facts();
    f.mic.as_mut().unwrap().muted = true;
    f
}

fn texts(nodes: &[Node]) -> Vec<(String, TextStyle, Ink)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text {
                text, style, ink, ..
            } => Some((text.to_string(), *style, *ink)),
            _ => None,
        })
        .collect()
}

fn strokes_at_y(nodes: &[Node], y: f64) -> Vec<(Ink, f64)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Stroke {
                points, ink, width, ..
            } if points.len() == 2 && points[0].1 == y && points[1].1 == y => Some((*ink, *width)),
            _ => None,
        })
        .collect()
}

#[test]
fn brightness_ruler_is_ink_not_flow() {
    let page = system_page(&facts(), "09:41");
    let row = strokes_at_y(&page.list.nodes, 80.0);
    assert!(row.contains(&(Ink::Ink, 2.0)));
    assert!(row.iter().all(|(ink, _)| *ink != Ink::Flow));
    let ink_station = page.list.nodes.iter().any(|n| {
        matches!(n, Node::Station { y, mark: StationMark::Filled, ink: Ink::Ink, .. } if *y == 80.0)
    });
    assert!(ink_station);
    let t = texts(&page.list.nodes);
    assert!(t.contains(&("70".into(), TextStyle::Clock, Ink::Ink)));
    assert!(t.contains(&("Parlaklık".into(), TextStyle::Name, Ink::Ink)));
    assert!(t.contains(&("DELL AW2725DF".into(), TextStyle::Sub, Ink::Faint)));
}

#[test]
fn brightness_click_sets_value() {
    let page = system_page(&facts(), "09:41");
    // The ruler spans x 200 to 550: 375 is the middle.
    assert_eq!(
        hit(&page.targets, 375.0, 80.0),
        Some(Action::SetBrightness(50))
    );
    assert_eq!(
        hit(&page.targets, 200.0, 80.0),
        Some(Action::SetBrightness(0))
    );
    assert_eq!(
        hit(&page.targets, 551.0, 80.0),
        Some(Action::SetBrightness(100))
    );
    assert_eq!(
        hit(&page.targets, 445.0, 80.0),
        Some(Action::SetBrightness(70))
    );
}

#[test]
fn brightness_unsupported() {
    let mut f = facts();
    f.brightness = None;
    let page = system_page(&f, "09:41");
    let t = texts(&page.list.nodes);
    assert!(t.contains(&(
        "DESTEKLENMİYOR · MONİTÖRDE DDC/CI KAPALI".to_string(),
        TextStyle::Label,
        Ink::Faint
    )));
    assert!(
        !page
            .targets
            .iter()
            .any(|t| matches!(t.action, Action::BrightnessRuler(_)))
    );
    assert!(strokes_at_y(&page.list.nodes, 80.0).is_empty());
}

#[test]
fn open_mic_is_a_flow_route_to_its_bus() {
    let page = system_page(&facts(), "09:41");
    assert!(strokes_at_y(&page.list.nodes, 132.0).contains(&(Ink::Flow, 2.0)));
    let t = texts(&page.list.nodes);
    assert!(t.iter().any(|(s, _, _)| s == "B1"));
    assert!(!t.iter().any(|(s, _, _)| s == "KAPALI"));
}

#[test]
fn muted_mic_is_cut_with_kapali() {
    let page = system_page(&muted(), "09:41");
    let row = strokes_at_y(&page.list.nodes, 132.0);
    assert_eq!(row, vec![(Ink::Track, 1.0), (Ink::Track, 1.0)]);
    let centred = page.list.nodes.iter().any(|n| {
        matches!(n, Node::Text { text, rect, align: Align::Center, .. }
            if &**text == "KAPALI" && (rect.x + rect.w / 2.0 - 345.0).abs() < 1e-9)
    });
    assert!(centred);
}

#[test]
fn mic_row_click_toggles() {
    for f in [facts(), muted()] {
        let page = system_page(&f, "09:41");
        for x in [60.0, 345.0, 580.0] {
            assert_eq!(hit(&page.targets, x, 132.0), Some(Action::ToggleMic));
        }
    }
}

#[test]
fn no_mic_hides_mic_row() {
    let mut f = facts();
    f.mic = None;
    let page = system_page(&f, "09:41");
    assert!(
        !texts(&page.list.nodes)
            .iter()
            .any(|(s, _, _)| s == "Mikrofon")
    );
    assert!(!page.targets.iter().any(|t| t.action == Action::ToggleMic));
    assert!(strokes_at_y(&page.list.nodes, 132.0).is_empty());
    assert_eq!(page.list.size.1, 178.0);
}

#[test]
fn system_page_is_178_high() {
    for f in [facts(), muted()] {
        let page = system_page(&f, "09:41");
        assert_eq!(page.list.size, (620.0, 178.0));
        assert_eq!(page.shape.height, 178.0);
        assert_eq!(page.shape.width, 620.0);
    }
}

#[test]
fn brightness_over_100_is_clamped_once() {
    let mut f = facts();
    f.brightness = Some(("DELL AW2725DF".into(), 130));
    let page = system_page(&f, "09:41");
    assert!(texts(&page.list.nodes).iter().any(|(s, _, _)| s == "100"));
    let fill_end = page.list.nodes.iter().find_map(|n| match n {
        Node::Station { x, y, .. } if *y == 80.0 => Some(*x),
        _ => None,
    });
    assert_eq!(fill_end, Some(550.0));
}
