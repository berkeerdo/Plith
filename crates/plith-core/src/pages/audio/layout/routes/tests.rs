#![allow(clippy::float_cmp)]

use crate::draw::{Ink, Node};
use crate::pages::audio::{PickMode, audio_page, audio_view};
use crate::pages::fixtures;

/// A polyline's points, ink and dash flag.
type Line = (Vec<(f64, f64)>, Ink, bool);

/// The polylines of `nodes` with their ink and dash flag.
fn strokes(nodes: &[Node]) -> Vec<Line> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Stroke {
                points,
                ink,
                dashed,
                ..
            } => Some((points.clone(), *ink, *dashed)),
            _ => None,
        })
        .collect()
}

fn page_strokes(facts: &crate::pages::audio::AudioFacts, mode: PickMode) -> Vec<Line> {
    let (activity, now) = fixtures::banana_activity();
    let view = audio_view(facts, &activity, mode, now);
    strokes(&audio_page(&view, fixtures::CLOCK).list.nodes)
}

#[test]
fn converging_routes_stagger_their_diagonals_like_the_mock() {
    // Mock lines 162 to 164: Windows and Discord join A1 on separate diagonals.
    let lines = page_strokes(&fixtures::banana(), PickMode::None);
    let flow: Vec<&Vec<(f64, f64)>> = lines
        .iter()
        .filter(|(_, ink, dashed)| *ink == Ink::Flow && !dashed)
        .map(|(p, _, _)| p)
        .collect();
    let windows = vec![(252.0, 183.0), (300.0, 183.0), (400.0, 83.0), (452.0, 83.0)];
    let discord = vec![(252.0, 233.0), (280.0, 233.0), (430.0, 83.0), (452.0, 83.0)];
    let mic = vec![(252.0, 83.0), (330.0, 83.0), (430.0, 183.0), (452.0, 183.0)];
    for wanted in [&windows, &discord, &mic] {
        assert!(flow.contains(&wanted), "missing {wanted:?} in {flow:?}");
    }
}

#[test]
fn candidates_run_on_a_rail_below_the_picked_source() {
    // Mock lines 171 to 173: dashed candidates leave from (XS + 8, y + 9) and end 12 DIP
    // before the output station; the same-row candidate runs straight under the lit line.
    let lines = page_strokes(&fixtures::banana(), PickMode::Picked(0));
    let dashed: Vec<&Vec<(f64, f64)>> = lines
        .iter()
        .filter(|(_, _, dashed)| *dashed)
        .map(|(p, _, _)| p)
        .collect();
    assert_eq!(dashed.len(), 3);
    assert!(dashed.contains(&&vec![(260.0, 92.0), (452.0, 92.0)]));
    assert!(dashed.contains(&&vec![
        (260.0, 92.0),
        (399.0, 92.0),
        (440.0, 133.0),
        (452.0, 133.0)
    ]));
    assert!(dashed.contains(&&vec![
        (260.0, 92.0),
        (299.0, 92.0),
        (440.0, 233.0),
        (452.0, 233.0)
    ]));
}

#[test]
fn reachable_endpoints_get_a_track_route_without_voicemeeter() {
    // Mock `audioNoVoicemeeter()`: the Edifier is reachable as a 1 px track route.
    let lines = page_strokes(&fixtures::no_voicemeeter(), PickMode::None);
    let track: Vec<&Vec<(f64, f64)>> = lines
        .iter()
        .filter(|(p, ink, _)| *ink == Ink::Track && p.len() == 4)
        .map(|(p, _, _)| p)
        .collect();
    assert_eq!(
        track,
        [&vec![
            (252.0, 83.0),
            (380.0, 83.0),
            (430.0, 133.0),
            (452.0, 133.0)
        ]]
    );
}
