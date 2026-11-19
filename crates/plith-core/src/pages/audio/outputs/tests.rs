#![allow(clippy::float_cmp)]

use crate::draw::{Ink, Node, StationMark, TextStyle, Trim};
use crate::lang::Lang;
use crate::pages::audio::{PickMode, audio_page, audio_view};
use crate::pages::fixtures::{self, CLOCK};
use crate::pages::{Action, PageBuild, hit};
use crate::shapes;

fn build(facts: &crate::pages::audio::AudioFacts) -> PageBuild {
    audio_page(
        &audio_view(facts, PickMode::None, Lang::Tr),
        CLOCK,
        Lang::Tr,
    )
}

fn text_node<'a>(nodes: &'a [Node], wanted: &str) -> Option<&'a Node> {
    nodes
        .iter()
        .find(|n| matches!(n, Node::Text { text, .. } if &**text == wanted))
}

fn ink_and_style(page: &PageBuild, wanted: &str) -> (Ink, TextStyle) {
    match text_node(&page.list.nodes, wanted) {
        Some(Node::Text { ink, style, .. }) => (*ink, *style),
        _ => panic!("{wanted} missing"),
    }
}

fn fill_ink(page: &PageBuild) -> Ink {
    page.list
        .nodes
        .iter()
        .find_map(|n| match n {
            Node::Stroke {
                width, ink, points, ..
            } if *width == 2.0 && points.len() == 2 && points[0].0 == 264.0 => Some(*ink),
            _ => None,
        })
        .expect("filled part")
}

#[test]
fn default_row_names_the_device_and_the_volume() {
    let page = build(&fixtures::no_voicemeeter());
    assert_eq!(
        ink_and_style(&page, "Logitech G733"),
        (Ink::Ink, TextStyle::Name)
    );
    assert_eq!(
        ink_and_style(&page, "VARSAYILAN \u{b7} KULAKLIK"),
        (Ink::Faint, TextStyle::Sub)
    );
    assert_eq!(ink_and_style(&page, "%62"), (Ink::Ink, TextStyle::Clock));
    assert_eq!(fill_ink(&page), Ink::Flow);
    let thumb = page.list.nodes.iter().find_map(|n| match n {
        Node::Station {
            x,
            y,
            mark: StationMark::Filled,
            ink,
        } if *y == 80.0 => Some((*x, *y, *ink)),
        _ => None,
    });
    let at = 264.0 + 266.0 * 0.62;
    assert_eq!(thumb, Some((at, 80.0, Ink::Flow)));
}

#[test]
fn muted_default_greys_the_ruler_and_says_sessiz() {
    let page = build(&fixtures::no_voicemeeter_muted());
    assert_eq!(fill_ink(&page), Ink::Faint);
    assert_eq!(
        ink_and_style(&page, "SESS\u{130}Z"),
        (Ink::Faint, TextStyle::Label)
    );
    assert!(text_node(&page.list.nodes, "%62").is_none());
}

#[test]
fn the_ruler_click_sets_the_volume() {
    let page = build(&fixtures::no_voicemeeter());
    assert_eq!(
        hit(&page.targets, 264.0 + 266.0 * 0.5, 80.0),
        Some(Action::SetVolume(50))
    );
    assert_eq!(hit(&page.targets, 264.0, 80.0), Some(Action::SetVolume(0)));
    assert_eq!(
        hit(&page.targets, 530.0, 80.0),
        Some(Action::SetVolume(100))
    );
}

#[test]
fn other_outputs_sit_under_their_label_and_become_default_on_click() {
    let page = build(&fixtures::no_voicemeeter());
    assert_eq!(
        ink_and_style(&page, "DİĞER ÇIKIŞLAR"),
        (Ink::Faint, TextStyle::Label)
    );
    let Some(Node::Text { trim, .. }) = text_node(&page.list.nodes, "Edifier R1280") else {
        panic!("row missing");
    };
    assert_eq!(*trim, Trim::Middle);
    assert_eq!(
        ink_and_style(&page, "Edifier R1280"),
        (Ink::Muted, TextStyle::Body)
    );
    assert_eq!(
        ink_and_style(&page, "HOPARLÖR"),
        (Ink::Faint, TextStyle::Sub)
    );
    assert_eq!(ink_and_style(&page, "EKRAN"), (Ink::Faint, TextStyle::Sub));
    assert_eq!(
        hit(&page.targets, 300.0, 146.0),
        Some(Action::SetDefaultEndpoint("r1280".into()))
    );
    assert_eq!(
        hit(&page.targets, 300.0, 186.0),
        Some(Action::SetDefaultEndpoint("dell".into()))
    );
    let below_ruler = page
        .list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Station { y, .. } if *y > 100.0))
        .count();
    assert_eq!(below_ruler, 0, "other rows draw no stations");
}

#[test]
fn the_value_text_clears_the_ruler_end_by_twelve() {
    for facts in [fixtures::no_voicemeeter(), fixtures::no_voicemeeter_muted()] {
        let page = build(&facts);
        for label in ["%62", "SESS\u{130}Z"] {
            if let Some(Node::Text { rect, .. }) = text_node(&page.list.nodes, label) {
                // End-aligned text: its start is the right edge minus its width.
                let width = if label == "%62" { 22.2 } else { 39.6 };
                let start = rect.x + rect.w - width;
                assert!(start - 530.0 >= 12.0, "{label} starts at {start}");
                assert_eq!(rect.x + rect.w, 590.0);
            }
        }
    }
}

#[test]
fn the_footer_names_the_click_and_not_voicemeeter() {
    let page = build(&fixtures::no_voicemeeter());
    assert_eq!(
        ink_and_style(&page, "ÇIKIŞA TIKLA: VARSAYILAN OLUR"),
        (Ink::Faint, TextStyle::Label)
    );
    for node in &page.list.nodes {
        if let Node::Text { text, .. } = node {
            assert!(!text.to_uppercase().contains("VOICEMEETER"));
        }
    }
}

#[test]
fn only_the_default_output_omits_the_other_label_and_stays_short() {
    let mut facts = fixtures::no_voicemeeter();
    facts.endpoints.truncate(1);
    let page = build(&facts);
    assert!(text_node(&page.list.nodes, "DİĞER ÇIKIŞLAR").is_none());
    assert_eq!(page.list.size.1, 146.0);
}

#[test]
fn no_endpoints_says_cikis_yok() {
    let page = build(&fixtures::no_voicemeeter_no_endpoints());
    assert_eq!(
        ink_and_style(&page, "ÇIKIŞ YOK"),
        (Ink::Faint, TextStyle::Name)
    );
    assert!(text_node(&page.list.nodes, "DİĞER ÇIKIŞLAR").is_none());
    assert!(text_node(&page.list.nodes, "ÇIKIŞA TIKLA: VARSAYILAN OLUR").is_none());
    assert_eq!(page.list.size.1, 112.0);
    assert!(
        !page
            .targets
            .iter()
            .any(|t| matches!(t.action, Action::VolumeRuler(_)))
    );
}

#[test]
fn without_a_default_every_endpoint_is_an_other_row() {
    let mut facts = fixtures::no_voicemeeter();
    facts.default_id = None;
    let page = build(&facts);
    assert!(text_node(&page.list.nodes, "ÇIKIŞ YOK").is_some());
    assert!(text_node(&page.list.nodes, "DİĞER ÇIKIŞLAR").is_some());
    for name in ["Logitech G733", "Edifier R1280", "DELL AW2725DF"] {
        assert_eq!(
            ink_and_style(&page, name),
            (Ink::Muted, TextStyle::Body),
            "{name}"
        );
    }
}

#[test]
fn many_outputs_fit_the_page_and_end_in_a_count() {
    let page = build(&fixtures::no_voicemeeter_many());
    let h = page.list.size.1;
    assert!(h <= shapes::PAGE_MAX_HEIGHT);
    // Eleven others, six rows fit: five show, `+6` takes the sixth slot.
    assert!(text_node(&page.list.nodes, "Speakers 3").is_some());
    assert!(text_node(&page.list.nodes, "Speakers 4").is_none());
    assert_eq!(ink_and_style(&page, "+6"), (Ink::Faint, TextStyle::Label));
    assert!(page.targets.iter().all(|t| t.rect.y + t.rect.h <= h));
    for node in &page.list.nodes {
        if let Node::Text { rect, text, .. } = node
            && text.starts_with("Speakers")
        {
            assert!(rect.y + rect.h <= h - 32.0, "{text} below the footer line");
        }
    }
}

#[test]
fn a_long_name_is_cut_in_the_middle_inside_its_column() {
    let long = "Hoparl\u{f6}r (Realtek High Definition Audio with a long name)";
    let mut facts = fixtures::no_voicemeeter();
    facts.endpoints[1].name = long.to_string();
    let page = build(&facts);
    let Some(Node::Text { rect, trim, .. }) = text_node(&page.list.nodes, long) else {
        panic!("name missing");
    };
    assert_eq!(*trim, Trim::Middle);
    assert!(
        rect.x + rect.w <= 256.0,
        "ends 8 DIP before the ruler start"
    );
}

#[test]
fn the_page_has_a_tab_bar_and_the_audio_tab_hit() {
    let page = build(&fixtures::no_voicemeeter());
    assert_eq!(
        hit(&page.targets, 64.0, 30.0),
        Some(Action::Tab(crate::pages::PageId::Audio))
    );
}
