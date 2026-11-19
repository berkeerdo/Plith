#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Node, Rect, Trim};
use crate::lang::Lang;
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
        sound: Some(SoundWords {
            text: "%62 · KULAKLIK".into(),
            muted: false,
        }),
        lang: Lang::Tr,
    }
}

/// The words of `sound_words`, for the cases that only care about the text.
fn words(value: Option<u8>, muted: bool, kind: &str, off: &str) -> Option<String> {
    sound_words(value, muted, kind, off, Lang::Tr).map(|s| s.text)
}

/// Only the sound is set, so the Karot right side falls to the sound branch.
fn sound_only(text: &str, muted: bool) -> IdleFacts {
    let mut f = facts();
    f.turn = None;
    f.now_sky = None;
    f.playing = None;
    f.sound = Some(SoundWords {
        text: text.into(),
        muted,
    });
    f
}

fn strokes(list: &DisplayList) -> Vec<&Node> {
    list.nodes
        .iter()
        .filter(|n| matches!(n, Node::Stroke { .. } | Node::Polygon { .. }))
        .collect()
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
fn karot_right_falls_back_weather_then_track_then_sound() {
    let right = |f: &IdleFacts| {
        let t = texts(&idle_pill(IdleLook::Karot, f));
        assert_eq!(t[0].1, "14:05");
        t.get(1).map(|r| {
            assert_eq!((r.2, r.3, r.4), (TextStyle::Value, Ink::Muted, Align::End));
            assert_eq!(r.0.x + r.0.w, 184.0);
            r.1.clone()
        })
    };
    let mut f = facts();
    f.turn = None;
    assert_eq!(right(&f).as_deref(), Some("9° BULUTLU"));
    f.now_sky = None;
    assert_eq!(
        right(&f).as_deref(),
        Some("Song"),
        "the playing title, never cased"
    );
    f.playing = None;
    assert_eq!(right(&f).as_deref(), Some("%62 · KULAKLIK"));
    f.sound = None;
    assert_eq!(right(&f), None);
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
    assert_eq!(idle_pill(IdleLook::Seal, &f), pill(&f.seal, f.lang));
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

#[test]
#[allow(clippy::cast_precision_loss)]
fn sound_words_keep_the_kind_only_when_it_fits() {
    assert_eq!(
        words(Some(62), false, "EKRAN", "KAPALI").as_deref(),
        Some("%62 · EKRAN")
    );
    assert_eq!(
        words(Some(100), false, "KULAKLIK", "KAPALI").as_deref(),
        Some("%100")
    );
    assert_eq!(
        words(Some(62), false, "HEADPHONES", "OFF").as_deref(),
        Some("%62")
    );
    assert_eq!(words(Some(62), false, "", "OFF").as_deref(), Some("%62"));
    let off = sound_words(Some(62), true, "KULAKLIK", "KAPALI", Lang::Tr).unwrap();
    assert_eq!((off.text.as_str(), off.muted), ("KAPALI", true));
    let on = sound_words(Some(62), false, "EKRAN", "KAPALI", Lang::Tr).unwrap();
    assert!(!on.muted);
    assert_eq!(
        words(None, true, "KULAKLIK", "KAPALI").as_deref(),
        Some("KAPALI")
    );
    assert_eq!(
        sound_words(None, false, "KULAKLIK", "KAPALI", Lang::Tr),
        None
    );
    for kind in [
        "KULAKLIK",
        "HOPARLÖR",
        "EKRAN",
        "HEADPHONES",
        "SPEAKER",
        "DISPLAY",
    ] {
        for v in [0, 9, 62, 100] {
            let s = words(Some(v), false, kind, "OFF").unwrap();
            let needed = s.chars().count() as f64 * VALUE_ADVANCE;
            let room = TURN_W - GLYPH_ROOM;
            assert!(needed <= room, "{s} needs {needed}, room is {room}");
        }
    }
}

#[test]
fn karot_sound_has_speaker_glyph() {
    let list = idle_pill(IdleLook::Karot, &sound_only("%62", false));
    let right_edge = RIGHT_EDGE - 3.0 * VALUE_ADVANCE - GLYPH_GAP;
    let y = shapes::REST.height / 2.0 - glyphs::GLYPH / 2.0;
    let expected = glyphs::glyph(
        DeviceKind::Speaker,
        right_edge - glyphs::GLYPH,
        y,
        Ink::Muted,
    );
    assert!(!expected.is_empty());
    let got = strokes(&list);
    assert_eq!(got.len(), expected.len());
    for (g, e) in got.into_iter().zip(&expected) {
        assert_eq!(g, e);
    }
}

#[test]
fn karot_glyph_only_on_sound_branch() {
    let mut f = sound_only("%62", false);
    f.playing = Some(("Song".into(), "Artist".into()));
    assert!(strokes(&idle_pill(IdleLook::Karot, &f)).is_empty());
    f.playing = None;
    f.now_sky = Some("9° BULUTLU".into());
    assert!(strokes(&idle_pill(IdleLook::Karot, &f)).is_empty());
    f.now_sky = None;
    f.turn = Some("YAĞMUR 15:30".into());
    assert!(strokes(&idle_pill(IdleLook::Karot, &f)).is_empty());
}

#[test]
fn muted_has_no_glyph() {
    let list = idle_pill(IdleLook::Karot, &sound_only("KAPALI", true));
    assert!(strokes(&list).is_empty());
    assert_eq!(texts(&list)[1].1, "KAPALI");
}

#[test]
fn sound_words_fit_leaves_glyph_room() {
    // 12 characters (91.2 DIP) still fit beside the glyph (room 92), 13 do not.
    assert_eq!(
        words(Some(62), false, "ABCDEF", "OFF").as_deref(),
        Some("%62 · ABCDEF")
    );
    assert_eq!(
        words(Some(62), false, "ABCDEFG", "OFF").as_deref(),
        Some("%62")
    );
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn the_turn_box_fits_the_longest_turn_text_without_an_ellipsis() {
    use crate::lang::sky_word;
    use crate::weather::Sky;
    for lang in [Lang::Tr, Lang::En] {
        for sky in [Sky::Clear, Sky::Cloud, Sky::Rain, Sky::Snow] {
            let text = format!("{} 15:30", sky_word(sky, lang));
            #[allow(clippy::cast_precision_loss)]
            let needed = text.chars().count() as f64 * VALUE_ADVANCE;
            assert!(needed <= TURN_W, "{text} needs {needed}, box is {TURN_W}");
        }
    }
    // The longest Turkish word, and the turn box stays clear of the time box.
    assert!(TURN_W >= "BULUTLU 15:30".chars().count() as f64 * VALUE_ADVANCE);
    const { assert!(PAD_LEFT + TIME_W <= RIGHT_EDGE - TURN_W) };
}
