#![allow(clippy::float_cmp)]

use super::*;
use crate::lang::Lang;
use crate::pages::audio::{DeviceKind, PickMode, RouteState, audio_view};

#[test]
fn banana_is_the_mock_diagram() {
    let view = audio_view(&lit(&banana()), PickMode::None, Lang::Tr);
    let names: Vec<&str> = view.sources.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        ["Mikrofon", "Hat giri\u{15f}i", "Windows", "Discord"]
    );
    let subs: Vec<&str> = view.sources.iter().map(|s| s.sub.as_str()).collect();
    assert_eq!(subs, ["HW 1", "HW 2", "VAIO", "AUX"]);
    let outputs: Vec<&str> = view.outputs.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(
        outputs,
        [
            "Logitech G733",
            "Edifier R1280",
            "Discord",
            "Kullan\u{131}lm\u{131}yor"
        ]
    );
    let live: Vec<(usize, usize)> = view
        .routes
        .iter()
        .filter(|r| r.state == RouteState::Live)
        .map(|r| (r.source, r.output))
        .collect();
    assert_eq!(live, [(0, 2), (2, 0), (3, 0)]);
    assert!(
        view.routes
            .iter()
            .any(|r| r.source == 1 && r.state == RouteState::Cut)
    );
}

#[test]
fn potato_has_eight_by_eight() {
    let view = audio_view(&lit(&potato()), PickMode::None, Lang::Tr);
    assert_eq!(view.sources.len(), 8);
    assert_eq!(view.outputs.len(), 8);
}

#[test]
fn no_voicemeeter_flags_the_headset_as_default() {
    let view = audio_view(&no_voicemeeter(), PickMode::None, Lang::Tr);
    assert!(view.sources.is_empty());
    assert_eq!(view.outputs.len(), 3);
    assert_eq!(view.outputs[0].name, "Logitech G733");
    assert!(view.outputs[0].default);
    assert_eq!(view.outputs[0].kind, DeviceKind::Headphones);
    assert_eq!(view.outputs[1].kind, DeviceKind::Speaker);
    assert_eq!(view.outputs[2].kind, DeviceKind::Display);
}

#[test]
fn seek_head_sits_at_one_forty_two() {
    let facts = media_playing();
    let timeline = facts.timeline.expect("timeline");
    let live = live_values();
    assert_eq!(timeline.fraction_at(live.now), 102.0 / 243.0);
}

#[test]
fn art_is_a_full_square() {
    let art = art();
    assert_eq!((art.width, art.height), (ART_SIZE, ART_SIZE));
    assert_eq!(art.bgra.len(), (ART_SIZE * ART_SIZE * 4) as usize);
    assert!(art.bgra.chunks(4).all(|p| p[3] == 255));
}

#[test]
fn traces_cover_every_strip() {
    let live = live_values();
    assert_eq!(live.traces.len(), 8);
    assert!(live.traces.iter().all(|t| t.0.len() == TRACE_SAMPLES));
    assert!(
        live.traces
            .iter()
            .flat_map(|t| &t.0)
            .all(|v| (0.0..=1.0).contains(v))
    );
}

#[test]
fn system_states_differ_only_where_named() {
    assert!(!system().mic.expect("mic").muted);
    assert!(system_mic_muted().mic.expect("mic").muted);
    assert!(system_no_ddc().brightness.is_none());
}

#[test]
fn pills_are_live_muted_idle() {
    let states: Vec<PillState> = pills().iter().map(|p| p.state).collect();
    assert_eq!(states, [PillState::Live, PillState::Muted, PillState::Idle]);
}

/// Every display list the fixtures build, the pills included, in Turkish.
fn every_list() -> Vec<crate::draw::DisplayList> {
    every_list_in(Lang::Tr)
}

/// Every display list the fixtures build, the pills included, in `lang`.
fn every_list_in(lang: Lang) -> Vec<crate::draw::DisplayList> {
    use crate::pages::system::system_page;
    use crate::pages::{audio::audio_page, media::media_page, pill::pill, shelf::shelf_page};
    let mut lists = Vec::new();
    for facts in [
        banana(),
        potato(),
        no_voicemeeter(),
        no_voicemeeter_muted(),
        no_voicemeeter_no_endpoints(),
        no_voicemeeter_many(),
    ] {
        for mode in [PickMode::None, PickMode::Picked(0)] {
            let view = audio_view(&lit(&facts), mode, lang);
            lists.push(audio_page(&view, CLOCK, lang).list);
        }
    }
    for facts in [media_playing(), media_empty()] {
        lists.push(media_page(&facts, CLOCK, lang).list);
    }
    for facts in [system(), system_mic_muted(), system_no_ddc()] {
        lists.push(system_page(&facts, CLOCK, lang).list);
    }
    lists.push(shelf_page(CLOCK, lang).list);
    lists.extend(pills().iter().map(|p| pill(p, lang)));
    lists
}

#[test]
fn hollow_stations_are_faint_or_candidates_like_the_mock() {
    // Mock `station()`: a hollow square is stroked faint, or lime for a candidate.
    use crate::draw::{Ink, Node, StationMark};
    for list in every_list() {
        for node in &list.nodes {
            if let Node::Station {
                mark: StationMark::Hollow,
                ink,
                x,
                y,
            } = node
            {
                assert!(
                    matches!(ink, Ink::Faint | Ink::Flow),
                    "hollow station at ({x}, {y}) is {ink:?}"
                );
            }
        }
    }
}

#[test]
fn output_rows_use_the_mock_sizes() {
    // Mock `audioPage()`: output names in sans 12, values in mono 10.
    use crate::draw::{Node, TextStyle};
    let view = audio_view(&lit(&banana()), PickMode::None, Lang::Tr);
    let page = crate::pages::audio::audio_page(&view, CLOCK, Lang::Tr);
    let style_of = |wanted: &str| {
        page.list.nodes.iter().find_map(|n| match n {
            Node::Text { text, style, .. } if &**text == wanted => Some(*style),
            _ => None,
        })
    };
    assert_eq!(style_of("Logitech G733"), Some(TextStyle::Body));
    assert_eq!(style_of("-6"), Some(TextStyle::Value));
    assert_eq!(style_of("Mikrofon"), Some(TextStyle::Name));
}

#[test]
fn named_buses_read_in_ink_and_the_unused_bus_stays_quiet() {
    // Mock `audioPage()`: A2 Edifier is ink with its value although nothing routes to it; only
    // the unused B2 is faint and shows no value.
    use crate::draw::{Ink, Node};
    let view = audio_view(&lit(&banana()), PickMode::None, Lang::Tr);
    let page = crate::pages::audio::audio_page(&view, CLOCK, Lang::Tr);
    let ink_of = |wanted: &str| {
        page.list.nodes.iter().find_map(|n| match n {
            Node::Text { text, ink, .. } if &**text == wanted => Some(*ink),
            _ => None,
        })
    };
    assert_eq!(ink_of("Edifier R1280"), Some(Ink::Ink));
    assert_eq!(ink_of("Kullan\u{131}lm\u{131}yor"), Some(Ink::Faint));
    let zero_db = page
        .list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Text { text, .. } if &**text == "0"))
        .count();
    assert_eq!(zero_db, 1, "B1 only; the unused B2 has no value");
}

#[test]
fn the_next_track_differs_in_title_art_and_timeline() {
    let (first, next) = (media_playing(), media_next());
    assert_ne!(first.title, next.title);
    assert_ne!(first.art, next.art);
    assert_ne!(
        first.timeline.map(|t| t.duration_s),
        next.timeline.map(|t| t.duration_s)
    );
    let second = art_next();
    assert_eq!(second.id, ART_NEXT_ID);
    assert_eq!(second.bgra.len(), (ART_SIZE * ART_SIZE * 4) as usize);
    assert_ne!(second.bgra, art().bgra);
}

/// Names that come from the fixtures' Voicemeeter labels and Windows devices, not from our copy.
const FIXTURE_NAMES: [&str; 2] = ["Mikrofon", "Hat giri\u{15f}i"];

/// Letters only Turkish uses; a page in another language must not show them in its own copy.
pub(crate) const TURKISH_ONLY: &str = "\u{11f}\u{11e}\u{131}\u{130}\u{15f}\u{15e}";

#[test]
fn no_page_leaks_turkish_copy_in_another_language() {
    use crate::draw::Node;
    for lang in [Lang::En, Lang::De] {
        let mut seen = 0;
        for list in every_list_in(lang) {
            for node in &list.nodes {
                let Node::Text { text, .. } = node else {
                    continue;
                };
                seen += 1;
                if FIXTURE_NAMES.iter().any(|n| text.contains(n)) {
                    continue;
                }
                assert!(
                    !text.chars().any(|c| TURKISH_ONLY.contains(c)),
                    "{lang:?} page shows Turkish copy: {text}"
                );
            }
        }
        assert!(seen > 100, "the scan saw the pages ({seen})");
    }
}
