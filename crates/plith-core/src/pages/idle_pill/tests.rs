#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Node, Rect, Trim};
use crate::pages::pill::{PillState, pill};

fn facts() -> IdleFacts {
    IdleFacts {
        time: "14:05".into(),
        turn: Some("YAĞMUR 15:30".into()),
        now_sky: Some("9° BULUTLU".into()),
        playing: Some(("Song".into(), "Artist".into())),
        seal: PillFacts {
            output: "G733".into(),
            value: Some(62),
            state: PillState::Live,
        },
        lang: Lang::Tr,
    }
}

type TextRow = (Rect, String, TextStyle, Ink, Align, Trim);

fn texts(list: &DisplayList) -> Vec<TextRow> {
    list.nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text {
                rect,
                text,
                style,
                ink,
                align,
                trim,
            } => Some((*rect, text.to_string(), *style, *ink, *align, *trim)),
            _ => None,
        })
        .collect()
}

#[test]
fn karot_time_left_turn_right() {
    let t = texts(&idle_pill(IdleLook::Karot, &facts()));
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].1, "14:05");
    assert_eq!(t[0].0.x, 16.0);
    assert_eq!(t[0].2, TextStyle::Clock);
    assert_eq!(t[0].3, Ink::Ink);
    assert_eq!(t[1].1, "YAĞMUR 15:30");
    assert_eq!(t[1].2, TextStyle::Value);
    assert_eq!(t[1].3, Ink::Muted);
    assert_eq!(t[1].4, Align::End);
    assert_eq!(t[1].0.x + t[1].0.w, 184.0);
}

#[test]
fn karot_falls_back_to_now_sky_then_nothing() {
    let mut f = facts();
    f.turn = None;
    let t = texts(&idle_pill(IdleLook::Karot, &f));
    assert_eq!(t.len(), 2);
    assert_eq!(t[1].1, "9° BULUTLU");
    f.now_sky = None;
    let t = texts(&idle_pill(IdleLook::Karot, &f));
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].1, "14:05");
}

#[test]
fn clock_is_centered() {
    let t = texts(&idle_pill(IdleLook::Clock, &facts()));
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].1, "14:05");
    assert_eq!(t[0].2, TextStyle::Clock);
    assert_eq!(t[0].4, Align::Center);
    assert_eq!(t[0].0.x + t[0].0.w / 2.0, 100.0);
}

#[test]
fn playing_has_one_flow_station_and_trimmed_text() {
    let list = idle_pill(IdleLook::Playing, &facts());
    let flow = list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Station { ink: Ink::Flow, .. }))
        .count();
    assert_eq!(flow, 1);
    let t = texts(&list);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].1, "Song \u{b7} Artist");
    assert_eq!(t[0].5, Trim::End);
    assert!(t[0].0.x + t[0].0.w <= 184.0);
}

#[test]
fn playing_without_artist_shows_title_only() {
    let mut f = facts();
    f.playing = Some(("Song".into(), String::new()));
    let t = texts(&idle_pill(IdleLook::Playing, &f));
    assert_eq!(t[0].1, "Song");
}

#[test]
fn playing_without_track_shows_time() {
    let mut f = facts();
    f.playing = None;
    let list = idle_pill(IdleLook::Playing, &f);
    let t = texts(&list);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].1, "14:05");
    assert!(
        list.nodes
            .iter()
            .all(|n| !matches!(n, Node::Station { .. }))
    );
}

#[test]
fn seal_equals_pill() {
    let f = facts();
    assert_eq!(idle_pill(IdleLook::Seal, &f), pill(&f.seal));
}

#[test]
fn plain_and_hidden_have_no_text() {
    let f = facts();
    let plain = idle_pill(IdleLook::Plain, &f);
    assert!(plain.nodes.is_empty());
    assert_eq!(plain.size, (200.0, 32.0));
    let hidden = idle_pill(IdleLook::Hidden, &f);
    assert!(hidden.nodes.is_empty());
    assert_eq!(hidden.size, (200.0, 3.0));
}

#[test]
fn rest_shape_hidden_is_a_sliver() {
    assert_eq!(rest_shape(IdleLook::Hidden), shapes::REST_HIDDEN);
    assert_eq!(rest_shape(IdleLook::Karot), shapes::REST);
    assert_eq!(shapes::REST_HIDDEN.height, 3.0);
}

#[test]
fn no_node_uses_flow_except_playing_station() {
    let f = facts();
    for look in [
        IdleLook::Karot,
        IdleLook::Clock,
        IdleLook::Plain,
        IdleLook::Hidden,
    ] {
        let list = idle_pill(look, &f);
        for n in &list.nodes {
            let flow = match n {
                Node::Text { ink, .. } | Node::Station { ink, .. } | Node::Stroke { ink, .. } => {
                    *ink == Ink::Flow
                }
                _ => false,
            };
            assert!(!flow, "{look:?}");
        }
    }
}
