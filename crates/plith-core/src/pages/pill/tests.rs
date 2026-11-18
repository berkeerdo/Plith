#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Ink, Node, Rect, StationMark, TextStyle};
use crate::lang::Lang;

fn facts(state: PillState) -> PillFacts {
    PillFacts {
        output: "G733".into(),
        value: Some(62),
        state,
    }
}

fn text_rect(list: &DisplayList, s: &str) -> Rect {
    list.nodes
        .iter()
        .find_map(|n| match n {
            Node::Text { text, rect, .. } if &**text == s => Some(*rect),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {s}"))
}

#[test]
fn pill_is_200_by_32() {
    for s in [PillState::Live, PillState::Muted, PillState::Idle] {
        assert_eq!(pill(&facts(s), Lang::Tr).size, (200.0, 32.0));
    }
}

#[test]
fn pill_slots_do_not_move_between_states() {
    let live = pill(&facts(PillState::Live), Lang::Tr);
    let idle = pill(&facts(PillState::Idle), Lang::Tr);
    let muted = pill(&facts(PillState::Muted), Lang::Tr);
    let name = text_rect(&live, "G733");
    assert_eq!(text_rect(&idle, "G733"), name);
    assert_eq!(text_rect(&muted, "G733"), name);
    let value = text_rect(&live, "62");
    assert_eq!(text_rect(&idle, "62"), value);
    assert_eq!(text_rect(&muted, "KAPALI"), value);
}

#[test]
fn pill_muted_is_cut_with_kapali() {
    let list = pill(&facts(PillState::Muted), Lang::Tr);
    let lines: Vec<_> = list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Stroke { ink, .. } => Some(*ink),
            _ => None,
        })
        .collect();
    assert_eq!(lines, vec![Ink::Track, Ink::Track]);
    assert!(list.nodes.iter().any(|n| matches!(
        n,
        Node::Text { text, style: TextStyle::Label, ink: Ink::Faint, .. } if &**text == "KAPALI"
    )));
    assert!(
        !list
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Text { text, .. } if &**text == "62"))
    );
}

#[test]
fn pill_live_is_flow_and_idle_is_hollow() {
    let live = pill(&facts(PillState::Live), Lang::Tr);
    assert!(
        live.nodes
            .iter()
            .any(|n| matches!(n, Node::Stroke { ink: Ink::Flow, width, .. } if *width == 2.0))
    );
    let filled = live
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::Station {
                    mark: StationMark::Filled,
                    ink: Ink::Flow,
                    ..
                }
            )
        })
        .count();
    assert_eq!(filled, 2);
    let idle = pill(&facts(PillState::Idle), Lang::Tr);
    let hollow = idle
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::Station {
                    mark: StationMark::Hollow,
                    ..
                }
            )
        })
        .count();
    assert_eq!(hollow, 2);
    assert!(
        !idle
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Stroke { ink: Ink::Flow, .. }))
    );
}

#[test]
fn a_muted_pill_reads_the_muted_word_of_its_language() {
    let word = |lang| {
        pill(&facts(PillState::Muted), lang)
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Text { text, .. } if &**text == crate::lang::text(crate::lang::Text::Off, lang)))
    };
    assert!(word(Lang::De));
    assert!(word(Lang::Tr));
    assert_ne!(
        crate::lang::text(crate::lang::Text::Off, Lang::De),
        crate::lang::text(crate::lang::Text::Off, Lang::Tr)
    );
}
