#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Align, Ink, Node, StationMark, TextStyle, Trim};
use crate::weather::{Hour, Sky, Stamp};

pub(super) const DAY: Day = (2026, 11, 17);
pub(super) const NEXT_DAY: Day = (2026, 11, 18);
pub(super) const NOW_MIN: u32 = 14 * 60 + 32;

pub(super) fn play(start_min: u32, end_min: u32, title: &str, artist: &str) -> Play {
    Play {
        start_min,
        end_min,
        title: title.into(),
        artist: artist.into(),
    }
}

pub(super) fn sky_at(hour: u8) -> Sky {
    match hour {
        0..=9 => Sky::Clear,
        15..=18 => Sky::Rain,
        _ => Sky::Cloud,
    }
}

/// Two days of hourly forecast: clear until 10, cloud, rain 15 to 18, cloud again.
pub(super) fn forecast(read_hour: u8) -> Forecast {
    let hours = [DAY, NEXT_DAY]
        .into_iter()
        .flat_map(|day| {
            (0..24u8).map(move |hour| Hour {
                day,
                hour,
                sky: sky_at(hour),
                temp_c: if hour == 17 { 7.0 } else { 9.0 },
                rain_pct: Some(if sky_at(hour) == Sky::Rain { 80 } else { 10 }),
            })
        })
        .collect();
    Forecast {
        hours,
        read_at: Stamp {
            day: DAY,
            hour: read_hour,
            minute: 0,
        },
    }
}

pub(super) fn facts() -> WelcomeFacts {
    WelcomeFacts {
        now: (DAY, NOW_MIN),
        date: "SAL 17 KAS".into(),
        plays: vec![
            play(546, 594, "Teardrop", "Massive Attack"),
            play(600, 642, "Holocene", "Bon Iver"),
            play(645, 684, "Retrograde", "James Blake"),
            play(792, 834, "Nightcall", "Kavinsky"),
            play(840, 872, "Avril 14th", "Aphex Twin"),
        ],
        playing: true,
        forecast: Some(forecast(14)),
        weather_on: true,
        span: RunSpan::Six,
        unit: TempUnit::C,
        head: HeadSlot::Auto,
        sound: Some(SoundHead {
            output: "G733".into(),
            kind: "KULAKLIK".into(),
            value: Some(62),
            muted: false,
            flowing: true,
        }),
        elapsed: Some((72, 125)),
        lang: Lang::Tr,
    }
}

pub(super) fn nodes(f: &WelcomeFacts, probe: Option<i32>) -> Vec<Node> {
    welcome_page(f, probe).list.nodes
}

pub(super) fn texts(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

pub(super) fn has_text(nodes: &[Node], s: &str) -> bool {
    texts(nodes).iter().any(|t| t == s)
}

/// The song band fills: y 136, height 8.
pub(super) fn song_fills(nodes: &[Node]) -> Vec<(f64, Ink)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Fill { rect, ink, .. } if rect.y == 136.0 && rect.h == 8.0 => {
                Some((rect.x, *ink))
            }
            _ => None,
        })
        .collect()
}

/// The closed outlines of the weather band: (dashed, left x, right x).
pub(super) fn outlines(nodes: &[Node]) -> Vec<(bool, f64, f64)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Stroke {
                points,
                dashed,
                ink: Ink::Track,
                ..
            } if points.len() == 5 && points[0].1 == 156.5 => {
                Some((*dashed, points[0].0, points[1].0))
            }
            _ => None,
        })
        .collect()
}

pub(super) fn flow_stations(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .filter(|n| matches!(n, Node::Station { ink: Ink::Flow, .. }))
        .count()
}

mod page;

#[test]
fn bit_sits_at_now() {
    let list = nodes(&facts(), None);
    // Six hours either side of 14:32: now is the middle of x 30 to 590.
    assert!(list.iter().any(|n| matches!(
        n,
        Node::Station {
            x: 310.0,
            y: 150.0,
            mark: StationMark::Filled,
            ink: Ink::Ink
        }
    )));
    assert!(list.iter().any(|n| matches!(
        n,
        Node::Stroke { points, ink: Ink::Ink, .. }
            if points == &vec![(310.0, 132.0), (310.0, 172.0)]
    )));
}

#[test]
fn only_the_playing_interval_is_flow() {
    let fills = song_fills(&nodes(&facts(), None));
    assert_eq!(fills.len(), 5);
    assert_eq!(fills.iter().filter(|(_, ink)| *ink == Ink::Flow).count(), 1);
    let flow_x = fills.iter().find(|(_, ink)| *ink == Ink::Flow).unwrap().0;
    assert!(fills.iter().all(|(x, _)| *x <= flow_x));
    assert!(
        fills
            .iter()
            .filter(|(_, ink)| *ink != Ink::Flow)
            .all(|(_, ink)| *ink == Ink::Muted)
    );
}

#[test]
fn paused_has_no_flow_and_says_durdu() {
    let mut f = facts();
    f.playing = false;
    let list = nodes(&f, None);
    assert!(song_fills(&list).iter().all(|(_, ink)| *ink != Ink::Flow));
    assert_eq!(flow_stations(&list), 0);
    assert!(has_text(&list, "Aphex Twin · DURDU"));
    assert!(!has_text(&nodes(&facts(), None), "Aphex Twin · DURDU"));
}

#[test]
fn forecast_sections_are_dashed_logged_are_solid() {
    let sections = outlines(&nodes(&facts(), None));
    let logged: Vec<_> = sections.iter().filter(|s| !s.0).collect();
    let forecast: Vec<_> = sections.iter().filter(|s| s.0).collect();
    assert!(!logged.is_empty() && !forecast.is_empty());
    assert!(logged.iter().all(|s| s.2 <= 310.5));
    assert!(forecast.iter().all(|s| s.1 >= 309.5));
}

#[test]
fn weather_off_has_no_weather_nodes() {
    let mut off = facts();
    off.weather_on = false;
    let mut none = facts();
    none.forecast = None;
    for f in [off, none] {
        let list = nodes(&f, None);
        assert!(outlines(&list).is_empty());
        assert!(!list.iter().any(|n| matches!(
            n,
            Node::Fill {
                ink: Ink::FloorHigh,
                ..
            }
        )));
        assert!(!texts(&list).iter().any(|t| t.contains('°')));
        assert!(!texts(&list).iter().any(|t| t.starts_with("OKUMA")));
        // The axis is solid to the bit and dashed after it.
        let axis: Vec<_> = list
            .iter()
            .filter_map(|n| match n {
                Node::Stroke { points, dashed, .. }
                    if points.len() == 2 && points[0].1 == 150.0 && points[1].1 == 150.0 =>
                {
                    Some((*dashed, points[0].0, points[1].0))
                }
                _ => None,
            })
            .collect();
        assert_eq!(axis, vec![(false, 30.0, 310.0), (true, 310.0, 590.0)]);
    }
}

#[test]
fn no_plays_says_bugun_calan_yok() {
    let mut f = facts();
    f.plays.clear();
    f.playing = false;
    let list = nodes(&f, None);
    assert!(has_text(&list, "BUGÜN ÇALAN YOK"));
    assert!(song_fills(&list).is_empty());
    assert!(!has_text(&nodes(&facts(), None), "BUGÜN ÇALAN YOK"));
}

#[test]
fn run_across_midnight_labels_wrap() {
    let mut f = facts();
    f.now = (DAY, 23 * 60 + 10);
    let mut labels: Vec<(f64, String)> = nodes(&f, None)
        .iter()
        .filter_map(|n| match n {
            Node::Text { rect, text, .. } if rect.y == 192.0 => Some((rect.x, text.to_string())),
            _ => None,
        })
        .collect();
    labels.sort_by(|a, b| a.0.total_cmp(&b.0));
    let got: Vec<&str> = labels.iter().map(|l| l.1.as_str()).collect();
    assert_eq!(got, ["18", "20", "22", "00", "02", "04"]);
}

#[test]
fn page_has_hero_clock_date_and_no_active_tab() {
    let page = welcome_page(&facts(), None);
    assert_eq!(page.shape, shapes::WELCOME);
    assert_eq!(page.list.size, (620.0, 232.0));
    assert_eq!(page.targets.len(), 4);
    assert!(page.list.nodes.iter().any(|n| matches!(
        n,
        Node::Text { text, style: TextStyle::Hero, .. } if &**text == "14:32"
    )));
    assert!(has_text(&page.list.nodes, "SAL 17 KAS"));
    assert!(!page.list.nodes.iter().any(|n| matches!(
        n,
        Node::Station {
            mark: StationMark::Filled,
            y: 34.0,
            ..
        }
    )));
}
