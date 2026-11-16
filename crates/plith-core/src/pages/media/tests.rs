#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Ink, LiveNode, Node, StationMark, TextStyle};
use crate::pages::{Action, hit};
use std::time::SystemTime;

fn timeline() -> SeekTimeline {
    SeekTimeline {
        position_s: 102.0,
        duration_s: 243.0,
        stamp: SystemTime::UNIX_EPOCH,
        playing: true,
    }
}

fn facts() -> MediaFacts {
    MediaFacts {
        has_session: true,
        source_name: "Spotify".into(),
        title: "Midnight City".into(),
        artist: "M83".into(),
        art: None,
        playing: true,
        can_seek: true,
        can_next: true,
        can_prev: true,
        timeline: Some(timeline()),
        output: Some("A1 \u{b7} G733".into()),
    }
}

fn texts(nodes: &[Node]) -> Vec<(String, TextStyle)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, style, .. } => Some((text.to_string(), *style)),
            _ => None,
        })
        .collect()
}

fn has_text(nodes: &[Node], wanted: &str) -> bool {
    texts(nodes).iter().any(|(t, _)| t == wanted)
}

fn seeks(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .filter(|n| matches!(n, Node::Live(LiveNode::Seek { .. })))
        .count()
}

#[test]
fn page_is_232_high() {
    let page = media_page(&facts(), "21:47");
    assert_eq!(page.list.size, (620.0, 232.0));
    assert_eq!(page.shape.height, 232.0);
    assert_eq!(page.shape.width, 620.0);
    assert_eq!(page.shape.radius, 24.0);
}

#[test]
fn no_session_shows_empty() {
    let page = media_page(
        &MediaFacts {
            output: Some("A1 \u{b7} G733".into()),
            ..MediaFacts::default()
        },
        "21:47",
    );
    assert!(has_text(&page.list.nodes, "\u{c7}alan bir \u{15f}ey yok"));
    assert_eq!(page.list.size, (620.0, 150.0));
    assert_eq!(seeks(&page.list.nodes), 0);
    let hollow = page
        .list
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::Station {
                    mark: StationMark::Hollow,
                    y,
                    ..
                } if *y == 124.0
            )
        })
        .count();
    assert_eq!(hollow, 2);
    assert!(
        page.targets
            .iter()
            .all(|t| matches!(t.action, Action::Tab(_)))
    );
}

#[test]
fn no_timeline_hides_ruler() {
    let page = media_page(
        &MediaFacts {
            timeline: None,
            ..facts()
        },
        "21:47",
    );
    assert_eq!(seeks(&page.list.nodes), 0);
    assert!(!has_text(&page.list.nodes, "1:42"));
    assert!(
        !page
            .targets
            .iter()
            .any(|t| matches!(t.action, Action::Seek(_)))
    );
    assert!(page.targets.iter().any(|t| t.action == Action::PlayPause));
}

#[test]
fn seek_fraction_from_click_x() {
    let r = RULER;
    assert_eq!(seek_fraction(r, r.x), 0.0);
    assert_eq!(seek_fraction(r, r.x + r.w), 1.0);
    assert_eq!(seek_fraction(r, r.x + r.w / 2.0), 0.5);
    assert_eq!(seek_fraction(r, r.x - 50.0), 0.0);
    assert_eq!(seek_fraction(r, r.x + r.w + 50.0), 1.0);
}

#[test]
fn ruler_click_yields_seek_action() {
    let page = media_page(&facts(), "21:47");
    let got = hit(&page.targets, RULER.x + RULER.w / 2.0, RULER.y + 5.0);
    assert!(matches!(got, Some(Action::Seek(_))));
}

#[test]
fn disabled_next_has_no_target() {
    let page = media_page(
        &MediaFacts {
            can_next: false,
            ..facts()
        },
        "21:47",
    );
    assert!(!page.targets.iter().any(|t| t.action == Action::Next));
    assert!(page.targets.iter().any(|t| t.action == Action::Previous));
    let enabled = media_page(&facts(), "21:47");
    assert!(enabled.targets.iter().any(|t| t.action == Action::Next));
}

#[test]
fn disabled_transport_is_faint() {
    let page = media_page(
        &MediaFacts {
            can_next: false,
            ..facts()
        },
        "21:47",
    );
    let faint_strokes = page
        .list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Stroke { ink: Ink::Faint, width, .. } if *width == 1.5))
        .count();
    assert!(faint_strokes >= 2);
}

#[test]
fn format_time_cases() {
    assert_eq!(format_time(0), "0:00");
    assert_eq!(format_time(102), "1:42");
    assert_eq!(format_time(3725), "1:02:05");
    assert_eq!(format_time(59), "0:59");
    assert_eq!(format_time(3600), "1:00:00");
}

#[test]
fn empty_title_shows_source_name() {
    let page = media_page(
        &MediaFacts {
            title: String::new(),
            artist: String::new(),
            ..facts()
        },
        "21:47",
    );
    let t = texts(&page.list.nodes);
    assert!(t.contains(&("Spotify".to_string(), TextStyle::Title)));
    assert!(!has_text(&page.list.nodes, "M83"));
}

#[test]
fn route_is_flow_when_playing_track_when_paused() {
    let route_ink = |playing: bool| {
        let page = media_page(&MediaFacts { playing, ..facts() }, "21:47");
        page.list.nodes.iter().find_map(|n| match n {
            Node::Stroke { points, ink, .. } if points.first() == Some(&(96.0, 196.0)) => {
                Some(*ink)
            }
            _ => None,
        })
    };
    assert_eq!(route_ink(true), Some(Ink::Flow));
    assert_eq!(route_ink(false), Some(Ink::Track));
}

#[test]
fn no_output_hides_route_row() {
    let page = media_page(
        &MediaFacts {
            output: None,
            ..facts()
        },
        "21:47",
    );
    assert!(!has_text(&page.list.nodes, "A1 \u{b7} G733"));
    assert!(!has_text(&page.list.nodes, "Spotify"));
}

#[test]
fn art_is_image_or_placeholder() {
    let with = media_page(
        &MediaFacts {
            art: Some(ArtId(7)),
            ..facts()
        },
        "21:47",
    );
    assert!(
        with.list
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Image { art: ArtId(7), .. }))
    );
    let without = media_page(&facts(), "21:47");
    assert!(without.list.nodes.iter().any(|n| matches!(
        n,
        Node::Fill {
            ink: Ink::FaintTrack,
            ..
        }
    )));
}
