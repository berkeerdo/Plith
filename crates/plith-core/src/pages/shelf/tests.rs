#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Ink, Node, StationMark, TextStyle};
use crate::pages::Action;

#[test]
fn shelf_empty_is_170_high() {
    let page = shelf_page("09:41");
    assert_eq!(page.list.size, (620.0, 170.0));
    assert_eq!(page.shape.height, 170.0);
}

#[test]
fn shelf_empty_copy_and_siding() {
    let page = shelf_page("09:41");
    let texts: Vec<_> = page
        .list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text {
                text, style, ink, ..
            } => Some((text.to_string(), *style, *ink)),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&("Dosyaları buraya sürükle".into(), TextStyle::Name, Ink::Ink)));
    assert!(texts.contains(&(
        "RAFTA BEKLER, İSTEDİĞİN YERE GERİ SÜRÜKLERSİN".into(),
        TextStyle::Label,
        Ink::Faint
    )));
    let hollow = page
        .list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Station { y, mark: StationMark::Hollow, .. } if *y == 146.0))
        .count();
    assert_eq!(hollow, 2);
    assert!(page.list.nodes.iter().any(|n| matches!(
        n,
        Node::Stroke {
            dashed: true,
            ink: Ink::Faint,
            ..
        }
    )));
}

#[test]
fn shelf_has_only_tab_targets() {
    let page = shelf_page("09:41");
    assert_eq!(page.targets.len(), 4);
    assert!(
        page.targets
            .iter()
            .all(|t| matches!(t.action, Action::Tab(_)))
    );
}
