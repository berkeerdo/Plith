#![allow(clippy::float_cmp)]

use super::*;

#[test]
fn head_reads_probe_past_and_future() {
    let past = texts(&nodes(&facts(), Some(10)));
    assert!(past.contains(&"Holocene · Bon Iver".to_string()));
    assert!(past.contains(&"10:00 – 11:00 · 2 ŞARKI".to_string()));
    assert!(past.contains(&"GEÇMİŞ".to_string()));
    let future = texts(&nodes(&facts(), Some(17)));
    assert!(future.contains(&"YAĞMUR · 7° · %80".to_string()));
    assert!(future.contains(&"17:00 · TAHMİN".to_string()));
    assert!(future.contains(&"OKUMA 14:00".to_string()));
}

#[test]
fn stale_reading_says_eski() {
    let mut f = facts();
    f.forecast = Some(forecast(11));
    let list = nodes(&f, None);
    assert!(has_text(&list, "OKUMA 11:00 · ESKİ"));
    assert!(has_text(&list, "9° · BULUTLU"));
    assert!(has_text(&list, "15:00 YAĞMUR · 9°"));
    let fresh = nodes(&facts(), None);
    assert!(has_text(&fresh, "OKUMA 14:00"));
    assert!(!texts(&fresh).iter().any(|t| t.contains("ESKİ")));
}

#[test]
fn long_title_is_trimmed_inside_the_head() {
    let mut f = facts();
    f.head = HeadSlot::Playing;
    f.plays.last_mut().unwrap().title = "A very long title ".repeat(12);
    let list = nodes(&f, None);
    let title = list
        .iter()
        .find_map(|n| match n {
            Node::Text {
                rect,
                style: TextStyle::Name,
                trim,
                align,
                ..
            } => Some((*rect, *trim, *align)),
            _ => None,
        })
        .expect("a title line");
    assert_eq!(title.1, Trim::End);
    assert_eq!(title.2, Align::End);
    assert!(title.0.x >= 300.0 && title.0.x + title.0.w <= 590.0);
}

#[test]
fn probe_hour_outside_band_is_none() {
    let f = facts();
    assert_eq!(probe_hour(&f, 310.0, 150.0), Some(14));
    assert_eq!(probe_hour(&f, 30.0, 126.0), Some(8));
    assert_eq!(probe_hour(&f, 100.0, 125.9), None);
    assert_eq!(probe_hour(&f, 100.0, 172.1), None);
    assert_eq!(probe_hour(&f, 29.9, 150.0), None);
    assert_eq!(probe_hour(&f, 590.0, 150.0), None);
    let mut day = facts();
    day.span = RunSpan::Day;
    assert_eq!(probe_hour(&day, 30.0, 150.0), Some(0));
    assert_eq!(probe_hour(&day, 589.0, 150.0), Some(23));
}

#[test]
fn auto_head_is_sound_when_weather_off() {
    let mut f = facts();
    assert!(has_text(&nodes(&f, None), "9° · BULUTLU"));
    assert!(!has_text(&nodes(&f, None), "G733"));
    f.weather_on = false;
    let list = nodes(&f, None);
    assert!(has_text(&list, "G733"));
    assert!(has_text(&list, "%62 · KULAKLIK"));
    assert!(has_text(&list, "VARSAYILAN ÇIKIŞ"));
    assert!(!has_text(&list, "9° · BULUTLU"));
    f.sound.as_mut().unwrap().muted = true;
    assert!(has_text(&nodes(&f, None), "KAPALI"));
}

#[test]
fn sound_head_station_is_flow_only_when_flowing() {
    let mut f = facts();
    f.head = HeadSlot::Sound;
    let head_station = |list: &[Node]| {
        list.iter().find_map(|n| match n {
            Node::Station {
                x,
                y: 74.0,
                mark: StationMark::Filled,
                ink,
            } => Some((*x, *ink)),
            _ => None,
        })
    };
    let flowing = head_station(&nodes(&f, None)).expect("a head station");
    assert_eq!(flowing, (586.5, Ink::Flow));
    f.sound.as_mut().unwrap().flowing = false;
    let idle = nodes(&f, None);
    assert_eq!(head_station(&idle).map(|s| s.1), Some(Ink::Ink));
    assert_eq!(flow_stations(&idle), 0);
}

#[test]
fn playing_head_hides_run_song_label() {
    let body = |list: &[Node]| {
        list.iter()
            .filter(|n| {
                matches!(
                    n,
                    Node::Text {
                        style: TextStyle::Body,
                        ..
                    }
                )
            })
            .count()
    };
    let mut f = facts();
    f.head = HeadSlot::Weather;
    assert_eq!(body(&nodes(&f, None)), 1);
    f.head = HeadSlot::Playing;
    let list = nodes(&f, None);
    assert_eq!(body(&list), 0);
    assert!(has_text(&list, "Avril 14th"));
    assert!(has_text(&list, "Aphex Twin"));
}

#[test]
fn probe_overrides_any_head_slot() {
    for slot in [
        HeadSlot::Auto,
        HeadSlot::Weather,
        HeadSlot::Sound,
        HeadSlot::Playing,
        HeadSlot::Empty,
    ] {
        let mut f = facts();
        f.head = slot;
        let list = nodes(&f, Some(10));
        assert!(has_text(&list, "GEÇMİŞ"), "{slot:?}");
        assert!(!has_text(&list, "G733"), "{slot:?}");
    }
    let mut f = facts();
    f.head = HeadSlot::Empty;
    assert!(
        !texts(&nodes(&f, None))
            .iter()
            .any(|t| t.starts_with("OKUMA"))
    );
}

fn bounds(n: &Node) -> Vec<(f64, f64)> {
    match n {
        Node::Fill { rect, .. } | Node::Text { rect, .. } => {
            vec![(rect.x, rect.y), (rect.x + rect.w, rect.y + rect.h)]
        }
        Node::Stroke { points, .. } | Node::Polygon { points, .. } => points.clone(),
        Node::Station { x, y, .. } => vec![(x - 4.0, y - 4.0), (x + 4.0, y + 4.0)],
        _ => Vec::new(),
    }
}

#[test]
fn no_node_outside_620x232() {
    let slots = [
        HeadSlot::Auto,
        HeadSlot::Weather,
        HeadSlot::Sound,
        HeadSlot::Playing,
        HeadSlot::Empty,
    ];
    let spans = [RunSpan::Three, RunSpan::Six, RunSpan::Day];
    let nows = [5, 700, NOW_MIN, 1439];
    for slot in slots {
        for span in spans {
            for now in nows {
                for probe in [None, Some(-3), Some(10), Some(23), Some(27)] {
                    let mut f = facts();
                    f.head = slot;
                    f.span = span;
                    f.now = (DAY, now);
                    f.plays.last_mut().unwrap().title = "L".repeat(200);
                    for n in nodes(&f, probe) {
                        for (x, y) in bounds(&n) {
                            assert!(
                                (0.0..=620.0).contains(&x) && (0.0..=232.0).contains(&y),
                                "{n:?} for {slot:?} {span:?} {now} {probe:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn artist_node(list: &[Node], artist: &str) -> Vec<TextStyle> {
    list.iter()
        .filter_map(|n| match n {
            Node::Text { text, style, .. } if &**text == artist => Some(*style),
            _ => None,
        })
        .collect()
}

#[test]
fn paused_playing_slot_keeps_the_run_label() {
    let mut f = facts();
    f.head = HeadSlot::Playing;
    f.playing = false;
    f.paused_last = true;
    let list = nodes(&f, None);
    assert!(has_text(&list, "Avril 14th"));
    assert!(has_text(&list, "Aphex Twin · DURDU"));
}

#[test]
fn probe_over_playing_slot_keeps_the_run_label() {
    let mut f = facts();
    f.head = HeadSlot::Playing;
    let list = nodes(&f, Some(10));
    assert!(has_text(&list, "Avril 14th"));
    assert!(has_text(&list, "Aphex Twin"));
}

#[test]
fn artists_are_never_set_in_an_upper_casing_style() {
    let mut run_label = facts();
    run_label.head = HeadSlot::Weather;
    let mut head = facts();
    head.head = HeadSlot::Playing;
    for f in [run_label, head] {
        let styles = artist_node(&nodes(&f, None), "Aphex Twin");
        assert!(!styles.is_empty());
        assert!(
            styles
                .iter()
                .all(|s| !crate::draw::style_spec(*s).uppercase)
        );
    }
    let mut paused = facts();
    paused.playing = false;
    paused.paused_last = true;
    let styles = artist_node(&nodes(&paused, None), "Aphex Twin · DURDU");
    assert!(
        !styles.is_empty()
            && styles
                .iter()
                .all(|s| !crate::draw::style_spec(*s).uppercase)
    );
}

#[test]
fn welcome_page_shows_no_turkish_letters_in_another_language() {
    for lang in [Lang::En, Lang::De] {
        let f = WelcomeFacts { lang, ..facts() };
        for probe in [None, Some(10), Some(17)] {
            for text in texts(&nodes(&f, probe)) {
                assert!(
                    !text
                        .chars()
                        .any(|c| "\u{11f}\u{11e}\u{131}\u{130}\u{15f}\u{15e}".contains(c)),
                    "{lang:?} welcome page shows Turkish copy: {text}"
                );
            }
        }
    }
}
