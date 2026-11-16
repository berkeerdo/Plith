#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Ink, LiveNode, LiveRail, Node, StationMark, TextStyle, Trim};
use crate::pages::audio::{
    Footer, OutputRow, OutputValue, PITCH_NORMAL, PickMode, Route, RouteState, SourceRow,
};
use crate::pages::{Action, hit};

fn source(name: &str, sub: &str, muted: bool) -> SourceRow {
    SourceRow {
        name: name.into(),
        sub: sub.into(),
        muted,
        picked: false,
    }
}

fn bus(tag: &str, name: &str, value: OutputValue, live: bool) -> OutputRow {
    OutputRow {
        tag: Some(tag.into()),
        name: name.into(),
        value,
        live,
        muted: false,
        id: None,
        used: true,
    }
}

fn route(source: usize, output: usize, state: RouteState) -> Route {
    Route {
        source,
        output,
        state,
    }
}

/// The mock's rest data: four sources, four outputs, three lit routes, a muted line in.
fn mock_view() -> AudioView {
    AudioView {
        sources: vec![
            source("Mikrofon", "HW 1", false),
            source("Hat giri\u{15f}i", "HW 2", true),
            source("Windows", "VAIO", false),
            source("Discord", "AUX", false),
        ],
        outputs: vec![
            bus("A1", "Logitech G733", OutputValue::Db(0.0), true),
            bus("A2", "Edifier R1280", OutputValue::Db(-6.4), false),
            bus("B1", "Discord", OutputValue::Db(0.0), true),
            OutputRow {
                used: false,
                ..bus(
                    "B2",
                    "Kullan\u{131}lm\u{131}yor",
                    OutputValue::Db(0.0),
                    false,
                )
            },
        ],
        routes: vec![
            route(0, 2, RouteState::Live),
            route(1, 0, RouteState::Cut),
            route(2, 0, RouteState::Live),
            route(3, 0, RouteState::Live),
        ],
        mode: PickMode::None,
        footer: Footer::Normal,
        row_pitch: PITCH_NORMAL,
    }
}

fn picking_view() -> AudioView {
    let mut view = mock_view();
    view.sources[0].picked = true;
    view.mode = PickMode::Picked(0);
    view.footer = Footer::Picking;
    view.routes.push(route(0, 1, RouteState::Candidate));
    view.routes.push(route(0, 3, RouteState::Candidate));
    view
}

fn endpoint(name: &str, id: &str, value: OutputValue) -> OutputRow {
    OutputRow {
        tag: None,
        name: name.into(),
        value,
        live: false,
        muted: false,
        id: Some(id.into()),
        used: false,
    }
}

fn windows_view(second_name: &str) -> AudioView {
    let mut first = endpoint("Logitech G733", "a", OutputValue::Percent(62));
    first.live = true;
    first.used = true;
    AudioView {
        sources: vec![source("Windows", "VARSAYILAN \u{c7}IKI\u{15e}", false)],
        outputs: vec![first, endpoint(second_name, "b", OutputValue::None)],
        routes: vec![route(0, 0, RouteState::Live)],
        mode: PickMode::None,
        footer: Footer::NoVoicemeeter,
        row_pitch: PITCH_NORMAL,
    }
}

fn rows(n: usize, pitch: f64) -> AudioView {
    AudioView {
        sources: (0..n)
            .map(|i| source(&format!("S{i}"), "HW", false))
            .collect(),
        outputs: (0..n)
            .map(|i| bus(&format!("A{i}"), "Out", OutputValue::Db(0.0), false))
            .collect(),
        routes: Vec::new(),
        mode: PickMode::None,
        footer: Footer::Normal,
        row_pitch: pitch,
    }
}

fn text_node<'a>(nodes: &'a [Node], wanted: &str) -> Option<&'a Node> {
    nodes
        .iter()
        .find(|n| matches!(n, Node::Text { text, .. } if &**text == wanted))
}

fn text_count(nodes: &[Node], wanted: &str) -> usize {
    nodes
        .iter()
        .filter(|n| matches!(n, Node::Text { text, .. } if &**text == wanted))
        .count()
}

/// A stroke's points, width, ink and dash flag.
type StrokeParts<'a> = (&'a Vec<(f64, f64)>, f64, Ink, bool);

fn strokes(nodes: &[Node]) -> Vec<StrokeParts<'_>> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Node::Stroke {
                points,
                width,
                ink,
                dashed,
            } => Some((points, *width, *ink, *dashed)),
            _ => None,
        })
        .collect()
}

fn station_at(nodes: &[Node], x: f64, y: f64) -> Option<(StationMark, Ink)> {
    nodes.iter().find_map(|n| match n {
        Node::Station {
            x: sx,
            y: sy,
            mark,
            ink,
        } if *sx == x && *sy == y => Some((*mark, *ink)),
        _ => None,
    })
}

#[test]
fn route_turns_at_45_degrees() {
    for (from, to) in [
        ((SOURCE_X, 83.0), (OUTPUT_X, 197.0)),
        ((SOURCE_X, 254.0), (OUTPUT_X, 83.0)),
    ] {
        let p = route_path(from, to);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], from);
        assert_eq!(p[3], to);
        assert_eq!(p[0].1, p[1].1, "first leg is horizontal");
        assert_eq!(p[2].1, p[3].1, "last leg is horizontal");
        let (dx, dy) = (p[2].0 - p[1].0, p[2].1 - p[1].1);
        assert!(dx > 0.0);
        assert!((dx - dy.abs()).abs() < 1e-9, "diagonal at 45 degrees");
        assert!(p[1].0 > from.0 && p[2].0 < to.0);
    }
}

#[test]
fn route_diagonal_ends_before_the_output_like_the_mock() {
    // Mock: route(XS, 236, 280, 86, XO): a 150 rise starts at x 280 and ends at 430.
    let p = route_path((SOURCE_X, 233.0), (OUTPUT_X, 83.0));
    assert_eq!(p[1].0, 280.0);
    assert_eq!(p[2].0, 430.0);
}

#[test]
fn straight_route_has_two_points() {
    let p = route_path((SOURCE_X, 83.0), (OUTPUT_X, 83.0));
    assert_eq!(p, vec![(SOURCE_X, 83.0), (OUTPUT_X, 83.0)]);
}

#[test]
fn banana_page_is_300_high() {
    let page = audio_page(&mock_view(), "21:47");
    assert_eq!(page.list.size, (620.0, 300.0));
    assert_eq!(page.shape.width, 620.0);
    assert_eq!(page.shape.height, 300.0);
    assert_eq!(page.shape.radius, 24.0);
    assert!(
        text_node(&page.list.nodes, "21:47").is_some(),
        "tab bar clock"
    );
}

#[test]
fn potato_page_fits_max_height() {
    let page = audio_page(&rows(8, row_pitch(8)), "21:47");
    assert!(page.list.size.1 <= shapes::PAGE_MAX_HEIGHT);
    assert_eq!(page.shape.height, page.list.size.1);
    let huge = audio_page(&rows(20, row_pitch(20)), "21:47");
    assert_eq!(huge.list.size.1, shapes::PAGE_MAX_HEIGHT);
}

#[test]
fn footer_sits_at_the_bottom() {
    let page = audio_page(&mock_view(), "21:47");
    let Some(Node::Text {
        rect, style, ink, ..
    }) = text_node(&page.list.nodes, Footer::Normal.text())
    else {
        panic!("footer missing");
    };
    assert_eq!((*style, *ink), (TextStyle::Label, Ink::Faint));
    assert_eq!(rect.x, 30.0);
    assert_eq!(rect.y + rect.h, 300.0 - 14.0);
}

#[test]
fn long_endpoint_name_truncates_in_middle() {
    let long = "Hoparl\u{f6}r (Realtek High Definition Audio with a long name)";
    let page = audio_page(&windows_view(long), "21:47");
    let Some(Node::Text {
        rect, trim, style, ..
    }) = text_node(&page.list.nodes, long)
    else {
        panic!("endpoint name missing");
    };
    assert_eq!(*trim, Trim::Middle);
    assert_eq!(*style, TextStyle::Name);
    assert!(rect.x + rect.w <= VALUE_RIGHT);
}

#[test]
fn live_route_is_two_px_flow() {
    let page = audio_page(&mock_view(), "21:47");
    let all = strokes(&page.list.nodes);
    let mic = all
        .iter()
        .find(|(p, ..)| p.first() == Some(&(SOURCE_X, 83.0)))
        .expect("microphone route");
    assert_eq!((mic.1, mic.2, mic.3), (2.0, Ink::Flow, false));
    assert_eq!(mic.0.last(), Some(&(OUTPUT_X, 83.0 + 2.0 * PITCH_NORMAL)));
    let mut idle = mock_view();
    idle.routes = vec![route(0, 1, RouteState::Idle)];
    let page = audio_page(&idle, "21:47");
    let line = strokes(&page.list.nodes)
        .into_iter()
        .find(|(p, ..)| p.first() == Some(&(SOURCE_X, 83.0)))
        .expect("idle route");
    assert_eq!((line.1, line.2, line.3), (1.0, Ink::Track, false));
}

#[test]
fn cut_route_is_a_stub_with_sessiz() {
    let page = audio_page(&mock_view(), "21:47");
    let y = 83.0 + PITCH_NORMAL;
    let stubs: Vec<_> = strokes(&page.list.nodes)
        .into_iter()
        .filter(|(p, ..)| p.first() == Some(&(SOURCE_X, y)))
        .collect();
    assert_eq!(stubs.len(), 1);
    assert_eq!(stubs[0].0, &vec![(SOURCE_X, y), (CUT_END_X, y)]);
    assert_eq!((stubs[0].1, stubs[0].2), (1.0, Ink::Track));
    assert_eq!(
        station_at(&page.list.nodes, CUT_END_X, y),
        Some((StationMark::Hollow, Ink::Faint))
    );
    let Some(Node::Text {
        rect, style, ink, ..
    }) = text_node(&page.list.nodes, "SESS\u{130}Z")
    else {
        panic!("SESSİZ missing");
    };
    assert_eq!(
        (*style, *ink, rect.x),
        (TextStyle::Label, Ink::Faint, 300.0)
    );
}

#[test]
fn stations_follow_route_states() {
    let page = audio_page(&mock_view(), "21:47");
    let n = &page.list.nodes;
    let y = |i: f64| 83.0 + i * PITCH_NORMAL;
    let lit = Some((StationMark::Filled, Ink::Flow));
    assert_eq!(station_at(n, SOURCE_X, y(0.0)), lit);
    assert_eq!(
        station_at(n, SOURCE_X, y(1.0)),
        Some((StationMark::Filled, Ink::Ink))
    );
    assert_eq!(station_at(n, OUTPUT_X, y(0.0)), lit);
    assert_eq!(
        station_at(n, OUTPUT_X, y(1.0)),
        Some((StationMark::Hollow, Ink::Faint))
    );
}

#[test]
fn pick_mode_draws_brackets_and_dashed_candidates() {
    let page = audio_page(&picking_view(), "21:47");
    let n = &page.list.nodes;
    let Some(Node::Text { ink, .. }) = text_node(n, "[ Mikrofon ]") else {
        panic!("picked name missing");
    };
    assert_eq!(*ink, Ink::Flow);
    let dashed: Vec<_> = strokes(n).into_iter().filter(|s| s.3).collect();
    assert_eq!(dashed.len(), 2);
    assert!(dashed.iter().all(|s| s.1 == 1.0 && s.2 == Ink::Flow));
    assert_eq!(
        station_at(n, OUTPUT_X, 83.0 + PITCH_NORMAL),
        Some((StationMark::Hollow, Ink::Flow))
    );
    assert!(text_node(n, Footer::Picking.text()).is_some());
}

#[test]
fn outputs_show_tag_name_and_value() {
    let mut view = mock_view();
    view.outputs[1].tag = Some("A2".into());
    view.outputs[1].name = "A2".into();
    view.outputs[2].muted = true;
    let page = audio_page(&view, "21:47");
    let n = &page.list.nodes;
    assert_eq!(text_count(n, "A2"), 1, "tag drawn once when it is the name");
    assert!(text_node(n, "A1").is_some());
    assert!(text_node(n, "Logitech G733").is_some());
    assert!(
        text_node(n, "-6 dB").is_some(),
        "renamed row keeps its value"
    );
    assert_eq!(text_count(n, "0 dB"), 2, "A1 and B2; B1 is muted");
    assert_eq!(text_count(n, "SESS\u{130}Z"), 2, "cut line and muted B1");
    let Some(Node::Text { rect, .. }) = text_node(n, "0 dB") else {
        panic!("value missing");
    };
    assert_eq!(rect.x + rect.w, VALUE_RIGHT);
    let page = audio_page(&windows_view("Edifier R1280"), "21:47");
    assert!(text_node(&page.list.nodes, "62").is_some());
    assert_eq!(format_value(OutputValue::Db(-6.4)), "-6 dB");
    assert_eq!(format_value(OutputValue::Db(-0.3)), "0 dB");
    assert_eq!(format_value(OutputValue::None), "");
}

#[test]
fn meters_and_traces_are_live_nodes() {
    let page = audio_page(&mock_view(), "21:47");
    let traces: Vec<_> = page
        .list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Live(LiveNode::Trace { rect, rail, index }) => Some((*rect, *rail, *index)),
            _ => None,
        })
        .collect();
    // The mock draws a trace (with its clip notch) and no separate meter; muted sources none.
    assert_eq!(
        traces.iter().map(|t| t.2).collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
    assert!(traces.iter().all(|t| t.1 == LiveRail::Strip));
    assert_eq!(
        (traces[0].0.x, traces[0].0.y, traces[0].0.w, traces[0].0.h),
        (168.0, 75.0, 64.0, 16.0)
    );
    let page = audio_page(&windows_view("Edifier R1280"), "21:47");
    assert!(
        !page.list.nodes.iter().any(|n| matches!(n, Node::Live(_))),
        "no levels without Voicemeeter"
    );
}

#[test]
fn hit_source_then_output_actions() {
    let page = audio_page(&mock_view(), "21:47");
    let t = &page.targets;
    let row = |i: f64| 86.0 + i * PITCH_NORMAL;
    assert_eq!(hit(t, 60.0, row(0.0) - 5.0), Some(Action::PickSource(0)));
    assert_eq!(
        hit(t, SOURCE_X, row(3.0) - 3.0),
        Some(Action::PickSource(3))
    );
    assert_eq!(hit(t, 500.0, row(2.0) - 5.0), Some(Action::PickOutput(2)));
    assert_eq!(hit(t, 350.0, row(1.0)), None, "no cancel while not picking");
    let page = audio_page(&picking_view(), "21:47");
    assert_eq!(
        hit(&page.targets, 350.0, row(1.0)),
        Some(Action::CancelPick)
    );
    assert_eq!(
        hit(&page.targets, 500.0, row(1.0) - 5.0),
        Some(Action::PickOutput(1))
    );
    let page = audio_page(&windows_view("Edifier R1280"), "21:47");
    assert_eq!(
        hit(&page.targets, 500.0, row(1.0) - 5.0),
        Some(Action::SetDefaultEndpoint("b".into()))
    );
    assert_eq!(hit(&page.targets, 60.0, row(0.0) - 5.0), None);
    assert_eq!(
        hit(&page.targets, 30.0, 30.0),
        Some(Action::Tab(PageId::Audio))
    );
}

#[test]
fn pitch_follows_the_mock_and_the_reach() {
    assert_eq!(PITCH_NORMAL, 50.0);
    assert_eq!(row_pitch(1), 50.0);
    assert_eq!(row_pitch(4), 50.0);
    assert_eq!(row_pitch(5), 47.0);
    assert_eq!(row_pitch(6), 40.0);
    assert!((row_pitch(8) - 270.0 / 7.0).abs() < 1e-9);
    assert_eq!(row_pitch(20), 32.0);
}

#[test]
fn mock_page_has_its_footer_at_284() {
    let page = audio_page(&mock_view(), "21:47");
    assert_eq!(page.list.size.1, 300.0);
    let Some(Node::Text { rect, .. }) = text_node(&page.list.nodes, Footer::Normal.text()) else {
        panic!("footer missing");
    };
    // Label text rects sit 10 above the baseline.
    assert_eq!(rect.y + 10.0, 284.0);
}

fn is_45(p: &[(f64, f64)]) -> bool {
    p.len() == 2 || (p.len() == 4 && ((p[2].0 - p[1].0) - (p[2].1 - p[1].1).abs()).abs() < 1e-9)
}

#[test]
fn banana_five_rows_keep_45_degrees() {
    let mut view = rows(5, row_pitch(5));
    view.routes = (0..5)
        .flat_map(|s| (0..5).map(move |o| route(s, o, RouteState::Live)))
        .collect();
    let page = audio_page(&view, "21:47");
    let lines = strokes(&page.list.nodes);
    let routes: Vec<_> = lines.iter().filter(|l| l.0[0].0 == SOURCE_X).collect();
    assert_eq!(routes.len(), 25);
    assert!(routes.iter().all(|l| is_45(l.0)));
}

#[test]
fn tall_rise_slides_the_diagonal_end_then_runs_steeper() {
    // A 180 rise does not fit before XO - 22: the end slides right, still at 45 degrees.
    let p = route_path((SOURCE_X, 83.0), (OUTPUT_X, 263.0));
    assert!(is_45(&p));
    assert!(p[2].0 > OUTPUT_X - 22.0 && p[2].0 <= OUTPUT_X - 6.0);
    assert_eq!(p[1].0, SOURCE_X + 6.0);
    // Above the 188 reach (only at Potato density) the documented fallback runs steeper.
    let p = route_path((SOURCE_X, 83.0), (OUTPUT_X, 353.0));
    assert_eq!(p[1], (SOURCE_X + 6.0, 83.0));
    assert_eq!(p[2], (OUTPUT_X - 6.0, 353.0));
}

#[test]
fn one_source_eight_endpoints_fit_the_shape() {
    let mut view = windows_view("Edifier R1280");
    for i in 2..8 {
        view.outputs.push(endpoint(
            &format!("Out {i}"),
            &format!("id{i}"),
            OutputValue::None,
        ));
    }
    view.row_pitch = row_pitch(8);
    let page = audio_page(&view, "21:47");
    let h = page.list.size.1;
    assert!(h <= shapes::PAGE_MAX_HEIGHT);
    let footer_top = h - 26.0;
    for n in &page.list.nodes {
        if let Node::Text { rect, text, .. } = n
            && text.starts_with("Out")
        {
            assert!(
                rect.y + rect.h <= footer_top,
                "row {text} overlaps the footer"
            );
        }
    }
}

#[test]
fn bus_muted_cut_keeps_the_source_clean() {
    let mut view = mock_view();
    view.outputs[1].muted = true;
    view.routes = vec![route(0, 2, RouteState::Live), route(0, 1, RouteState::Cut)];
    view.sources[1].muted = false;
    let page = audio_page(&view, "21:47");
    let n = &page.list.nodes;
    let lines = strokes(n);
    let from = (SOURCE_X, 83.0);
    let live = lines
        .iter()
        .find(|l| l.0[0] == from && l.2 == Ink::Flow)
        .expect("live");
    assert_eq!(
        live.0,
        &route_path(from, (OUTPUT_X, 83.0 + 2.0 * PITCH_NORMAL))
    );
    let cut = lines
        .iter()
        .find(|l| l.0[0] == from && l.2 == Ink::Track)
        .expect("cut route");
    assert_eq!(cut.0, &route_path(from, (OUTPUT_X, 83.0 + PITCH_NORMAL)));
    assert_eq!(cut.1, 1.0);
    let cut_at = n.iter().position(|x| {
        matches!(
            x,
            Node::Stroke {
                ink: Ink::Track,
                ..
            }
        )
    });
    let live_at = n
        .iter()
        .position(|x| matches!(x, Node::Stroke { ink: Ink::Flow, .. }));
    assert!(cut_at < live_at, "cut drawn under the lit line");
    assert_eq!(text_count(n, "SESSİZ"), 1, "only the muted output value");
    assert_eq!(station_at(n, CUT_END_X, 83.0), None);
}

#[test]
fn unused_and_non_default_outputs_are_quiet() {
    let page = audio_page(&mock_view(), "21:47");
    let Some(Node::Text { ink, .. }) = text_node(&page.list.nodes, "Kullanılmıyor") else {
        panic!("unused bus missing");
    };
    assert_eq!(*ink, Ink::Faint);
    let page = audio_page(&windows_view("Edifier R1280"), "21:47");
    let ink_of = |name: &str| match text_node(&page.list.nodes, name) {
        Some(Node::Text { ink, .. }) => *ink,
        _ => panic!("{name} missing"),
    };
    assert_eq!(ink_of("Edifier R1280"), Ink::Muted);
    assert_eq!(ink_of("Logitech G733"), Ink::Ink);
}

#[test]
fn no_voicemeeter_muted_default_says_sessiz_only_at_the_output() {
    let mut view = windows_view("Edifier R1280");
    view.sources[0].muted = true;
    view.outputs[0].muted = true;
    view.outputs[0].live = false;
    view.routes = vec![route(0, 0, RouteState::Cut)];
    let page = audio_page(&view, "21:47");
    let n = &page.list.nodes;
    assert_eq!(text_count(n, "SESSİZ"), 1);
    let Some(Node::Text { rect, .. }) = text_node(n, "SESSİZ") else {
        unreachable!()
    };
    assert!(rect.x > OUTPUT_X, "the label is the output value");
}
