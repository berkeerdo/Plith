#![allow(clippy::float_cmp)]

use super::*;
use crate::draw::{Ink, Node, StationMark, TextStyle};
use crate::pages::{Action, PageId};
use crate::shelf::ShelfItem;

#[test]
fn shelf_empty_is_170_high() {
    let page = shelf_page("09:41", Lang::Tr);
    assert_eq!(page.list.size, (620.0, 170.0));
    assert_eq!(page.shape.height, 170.0);
}

#[test]
fn shelf_empty_copy_and_siding() {
    let page = shelf_page("09:41", Lang::Tr);
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
    let page = shelf_page("09:41", Lang::Tr);
    assert_eq!(page.targets.len(), 6, "home, four tabs and the gear");
    assert!(
        page.targets
            .iter()
            .all(|t| matches!(t.action, Action::Tab(_)))
    );
}

#[test]
fn the_pre_feed_default_matches_the_built_shelf() {
    assert_eq!(shelf_page("09:41", Lang::Tr).shape, crate::shapes::PAGES[2]);
}

fn items(n: usize) -> Vec<ShelfItem> {
    (0..n)
        .map(|i| ShelfItem {
            path: std::path::PathBuf::from(format!("C:\\Users\\a\\Proje gorseli {i}.png")),
            dir: false,
        })
        .collect()
}

fn texts(page: &PageBuild) -> Vec<String> {
    page.list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

fn page(n: usize, hot: bool, can_undo: bool) -> PageBuild {
    let items = items(n);
    shelf_page_with(
        &ShelfView {
            items: &items,
            hot,
            can_undo,
        },
        "09:41",
        Lang::Tr,
    )
}

#[test]
fn the_empty_shelf_with_an_undo_gets_room_below_the_siding() {
    let p = page(0, false, true);
    assert_eq!(p.list.size, (620.0, 198.0));
    let undo = p
        .targets
        .iter()
        .find(|t| t.action == Action::ShelfUndo)
        .unwrap();
    assert!(undo.rect.y > 146.0 + 8.0 && undo.rect.y + undo.rect.h < 198.0);
}

#[test]
fn a_folder_wagon_carries_the_localized_word() {
    let items = [ShelfItem {
        path: std::path::PathBuf::from("C:\\Users\\a\\Projeler"),
        dir: true,
    }];
    let view = ShelfView {
        items: &items,
        hot: false,
        can_undo: false,
    };
    let p = shelf_page_with(&view, "09:41", Lang::Tr);
    assert!(texts(&p).contains(&"KLASÖR".to_string()));
}

#[test]
fn the_page_grows_a_row_with_the_fifth_wagon() {
    for (n, height) in [(1, 214.0), (4, 214.0), (5, 338.0), (8, 338.0)] {
        let p = page(n, false, false);
        assert_eq!(p.list.size, (620.0, height), "{n} items");
        assert_eq!(p.shape.height, height);
    }
}

#[test]
fn a_wagon_shows_its_tag_name_and_the_footer() {
    let all = texts(&page(2, false, false));
    assert!(all.contains(&"PNG".to_string()));
    assert!(all.contains(&"Proje".to_string()), "{all:?}");
    assert!(all.contains(&"gorseli 0.png".to_string()));
    assert!(all.contains(&"BIRAK".to_string()));
    assert!(all.contains(&"2 DOSYA · SÜRÜKLE, BIRAK".to_string()));
    assert!(all.contains(&"GERİ AL".to_string()));
}

#[test]
fn every_wagon_has_two_filled_stations_on_its_track() {
    let p = page(5, false, false);
    let filled = p
        .list
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::Station {
                    mark: StationMark::Filled,
                    ..
                }
            )
        })
        .count();
    // Five wagons, two each, plus the filled tab station.
    assert_eq!(filled, 10 + 1);
}

#[test]
fn the_slot_is_dashed_faint_and_hot_while_a_drag_hovers() {
    let slot_ink = |p: &PageBuild| {
        p.list.nodes.iter().find_map(|n| match n {
            Node::Stroke {
                dashed: true, ink, ..
            } => Some(*ink),
            _ => None,
        })
    };
    assert_eq!(slot_ink(&page(3, false, false)), Some(Ink::Faint));
    assert_eq!(slot_ink(&page(3, true, false)), Some(Ink::Flow));
    assert_eq!(slot_ink(&page(0, true, false)), Some(Ink::Flow));
}

#[test]
fn a_press_on_a_station_removes_and_on_the_body_picks_the_wagon() {
    let p = page(2, false, false);
    let wagon = crate::shelf::layout::layout(2).wagons[1];
    let on_station = crate::pages::hit(&p.targets, wagon.stations[0], wagon.track_y);
    assert_eq!(on_station, Some(Action::ShelfRemove(1)));
    let on_body = crate::pages::hit(&p.targets, wagon.rect.x + 50.0, wagon.rect.y + 40.0);
    assert_eq!(on_body, Some(Action::ShelfWagon(1)));
    assert_eq!(crate::pages::hit(&p.targets, 400.0, 100.0), None);
}

#[test]
fn undo_is_a_target_only_when_there_is_something_to_undo() {
    let has_undo = |p: &PageBuild| p.targets.iter().any(|t| t.action == Action::ShelfUndo);
    assert!(!has_undo(&page(2, false, false)));
    assert!(has_undo(&page(2, false, true)));
    assert!(!has_undo(&page(0, false, false)));
    assert!(has_undo(&page(0, false, true)));
}

#[test]
fn the_undo_target_sits_on_the_undo_word() {
    let p = page(3, false, true);
    let undo = p
        .targets
        .iter()
        .find(|t| t.action == Action::ShelfUndo)
        .unwrap();
    let plan = crate::shelf::layout::layout(3);
    assert!(undo.rect.y <= plan.footer_y && plan.footer_y <= undo.rect.y + undo.rect.h + 6.0);
    assert!(undo.rect.x + undo.rect.w <= 590.0 + 0.001);
}

#[test]
fn the_tab_targets_stay_on_a_page_with_wagons() {
    let p = page(8, false, false);
    assert!(
        p.targets
            .iter()
            .any(|t| t.action == Action::Tab(PageId::Audio))
    );
}
