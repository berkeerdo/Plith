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
            .any(|t| matches!(t.action, Action::Seek(_) | Action::SeekRuler(_)))
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
    let got = hit(&page.targets, RULER.x + RULER.w * 0.25, RULER.y + 5.0);
    assert_eq!(got, Some(Action::Seek(0.25)));
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
        .filter(|n| matches!(n, Node::Stroke { ink: Ink::Faint, width, .. } if *width == 1.0))
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
    assert!(!t.iter().any(|(_, s)| *s == TextStyle::Body));
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

fn seek_targets(page: &PageBuild) -> usize {
    page.targets
        .iter()
        .filter(|t| matches!(t.action, Action::Seek(_) | Action::SeekRuler(_)))
        .count()
}

#[test]
fn cannot_seek_draws_ruler_without_target() {
    let page = media_page(
        &MediaFacts {
            can_seek: false,
            ..facts()
        },
        "21:47",
    );
    assert_eq!(seeks(&page.list.nodes), 1);
    assert_eq!(seek_targets(&page), 0);
}

#[test]
fn zero_duration_is_no_timeline() {
    let mut zero = timeline();
    zero.duration_s = 0.0;
    let page = media_page(
        &MediaFacts {
            timeline: Some(zero),
            ..facts()
        },
        "21:47",
    );
    assert_eq!(seeks(&page.list.nodes), 0);
    assert_eq!(seek_targets(&page), 0);
    assert!(!has_text(&page.list.nodes, "1:42"));
}

#[test]
fn enabled_transport_is_filled_ink() {
    // Mock lines 204 to 206: 1 DIP skip bars and filled triangles (fill plus a 1 DIP stroke).
    let page = media_page(&facts(), "21:47");
    let n = &page.list.nodes;
    let polygons: Vec<(&Vec<(f64, f64)>, Ink)> = n
        .iter()
        .filter_map(|n| match n {
            Node::Polygon { points, ink } => Some((points, *ink)),
            _ => None,
        })
        .collect();
    let cx = RULER.x + RULER.w / 2.0;
    assert_eq!(
        polygons,
        vec![
            (
                &vec![(cx - 38.0, 144.0), (cx - 48.0, 150.0), (cx - 38.0, 156.0)],
                Ink::Ink
            ),
            (
                &vec![(cx + 38.0, 144.0), (cx + 48.0, 150.0), (cx + 38.0, 156.0)],
                Ink::Ink
            ),
        ]
    );
    let bars = n
        .iter()
        .filter(|n| matches!(n, Node::Stroke { ink: Ink::Ink, width, points, .. } if *width == 1.0 && points.len() == 2 && points[0].0 == points[1].0))
        .count();
    assert_eq!(bars, 2);
    let paused = media_page(
        &MediaFacts {
            playing: false,
            ..facts()
        },
        "21:47",
    );
    let play = paused
        .list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Polygon { ink: Ink::Ink, .. }))
        .count();
    assert_eq!(play, 3, "two skips and the play triangle");
}

#[test]
fn empty_state_uses_strong_and_body() {
    let page = media_page(&MediaFacts::default(), "21:47");
    let t = texts(&page.list.nodes);
    assert!(t.contains(&("Çalan bir şey yok".to_string(), TextStyle::Strong)));
    assert!(t.iter().any(|(_, s)| *s == TextStyle::Body));
}

#[test]
fn artist_uses_body_style() {
    let page = media_page(&facts(), "21:47");
    assert!(texts(&page.list.nodes).contains(&("M83".to_string(), TextStyle::Body)));
}

#[test]
fn transport_wins_over_seek_where_they_touch() {
    let page = media_page(&facts(), "21:47");
    let y = 150.0 - 10.0 - 1.0;
    let got = hit(&page.targets, RULER.x + RULER.w / 2.0 - 2.0, y);
    assert_eq!(got, Some(Action::PlayPause));
}

#[test]
fn a_playing_seek_is_live_with_the_flow_ink() {
    let page = media_page(&facts(), "21:47");
    let inks: Vec<Ink> = page
        .list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Live(LiveNode::Seek { ink, .. }) => Some(*ink),
            _ => None,
        })
        .collect();
    assert_eq!(inks, [Ink::Flow]);
}

#[test]
fn a_paused_seek_is_drawn_statically_in_ink() {
    let paused = MediaFacts {
        playing: false,
        timeline: Some(SeekTimeline {
            playing: false,
            ..timeline()
        }),
        ..facts()
    };
    let page = media_page(&paused, "21:47");
    // Nothing moves on a paused page, so it has no live node and wants no ambient frames.
    assert_eq!(seeks(&page.list.nodes), 0);
    assert!(!page.list.nodes.iter().any(|n| matches!(n, Node::Live(_))));
    let head = RULER.x + RULER.w * 102.0 / 243.0;
    let y = RULER_Y;
    assert!(page.list.nodes.iter().any(|n| matches!(n,
        Node::Stroke { points, width, ink: Ink::Ink, dashed: false }
            if *width == 2.0 && points.len() == 2 && points[0] == (RULER.x, y)
                && (points[1].0 - head).abs() < 1e-9 && points[1].1 == y)));
    assert!(page.list.nodes.iter().any(|n| matches!(n,
        Node::Station { x, y: sy, mark: StationMark::Filled, ink: Ink::Ink }
            if (*x - head).abs() < 1e-9 && *sy == y)));
}
