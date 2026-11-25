use super::*;
use crate::draw::{Ink, Node, Rect, StationMark, TextStyle};
use crate::lang::sheet::{SheetText, sheet_text};
use crate::pages::idle_pill::{IdleFacts, idle_pill};
use crate::pages::pill::{PillFacts, PillState};
use crate::settings::{FlyoutPref, HeadSlot, IdleLook, LangChoice, LocationMode, RunSpan};

const LOOKS: [IdleLook; 6] = [
    IdleLook::Karot,
    IdleLook::Clock,
    IdleLook::Playing,
    IdleLook::Seal,
    IdleLook::Plain,
    IdleLook::Hidden,
];

fn facts_in(lang: Lang) -> SheetFacts {
    let idle = IdleFacts {
        time: "14:05".into(),
        turn: Some("YAGMUR 15:30".into()),
        now_sky: None,
        playing: Some(("Song".into(), "Artist".into())),
        seal: PillFacts {
            output: "G733".into(),
            value: Some(62),
            state: PillState::Live,
        },
        sound: None,
        lang,
        scroll_start: None,
    };
    SheetFacts {
        settings: Settings::default(),
        lang,
        previews: LOOKS.map(|l| Arc::new(idle_pill(l, &idle))),
        version: "0.4.0",
        build_date: "2026-11-20",
        flyout_locked: false,
        settings_path: "C:\\Users\\someone\\AppData\\Local\\Plith\\settings.txt".into(),
        save_failed: false,
        autostart: false,
    }
}

pub(super) fn facts() -> SheetFacts {
    facts_in(Lang::Tr)
}

pub(super) fn view(section: Section) -> SheetView {
    SheetView {
        section,
        ..SheetView::default()
    }
}

fn texts(list: &DisplayList) -> Vec<String> {
    list.nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

fn target_for<'a>(targets: &'a [SheetTarget], action: &SheetAction) -> &'a SheetTarget {
    targets
        .iter()
        .find(|t| &t.action == action)
        .unwrap_or_else(|| panic!("no target for {action:?}"))
}

fn center(t: &SheetTarget) -> (f64, f64) {
    (t.rect.x + t.rect.w / 2.0, t.rect.y + t.rect.h / 2.0)
}

fn click(targets: &[SheetTarget], at: (f64, f64)) -> Option<SheetAction> {
    sheet_hit(targets, at.0, at.1).map(|t| t.action.clone())
}

#[test]
fn five_sections_in_sidebar() {
    let (list, targets) = sheet_build(&facts(), &view(Section::Idle));
    for s in Section::ALL {
        assert!(
            targets.iter().any(|t| t.action == SheetAction::Section(s)),
            "{s:?}"
        );
    }
    let filled = list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Station {
                x,
                y,
                mark: StationMark::Filled,
                ..
            } if *x < 156.0 => Some(*y),
            _ => None,
        })
        .collect::<Vec<_>>();
    let idle = target_for(&targets, &SheetAction::Section(Section::Idle)).rect;
    assert_eq!(filled.len(), 1);
    assert!(filled[0] > idle.y && filled[0] < idle.y + idle.h);
}

#[test]
fn idle_rows_in_order() {
    let (list, _) = sheet_build(&facts(), &view(Section::Idle));
    let all = texts(&list);
    let mut at = 0;
    for label in [
        "KAPALI ADA",
        "\u{15e}erit",
        "Sa\u{11f} \u{fc}st",
        "Bug\u{fc}n \u{e7}alanlar",
        "Hava",
        "Konum",
        "Birim",
    ] {
        let found = all[at..].iter().position(|t| t == label);
        assert!(found.is_some(), "{label} missing or out of order");
        at += found.unwrap() + 1;
    }
}

#[test]
fn six_preview_insets() {
    let (list, _) = sheet_build(&facts(), &view(Section::Idle));
    let insets = list
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Inset { scale, .. } if (*scale - 0.45).abs() < 1e-9))
        .count();
    assert_eq!(insets, 6);
    let (list, _) = sheet_build(&facts(), &view(Section::General));
    assert!(!list.nodes.iter().any(|n| matches!(n, Node::Inset { .. })));
}

#[test]
fn preview_click_picks_the_look() {
    let (_, targets) = sheet_build(&facts(), &view(Section::Idle));
    let t = target_for(
        &targets,
        &SheetAction::Change(SettingChange::Look(IdleLook::Seal)),
    );
    assert_eq!(
        click(&targets, center(t)),
        Some(SheetAction::Change(SettingChange::Look(IdleLook::Seal)))
    );
}

#[test]
fn click_span_changes() {
    let (list, targets) = sheet_build(&facts(), &view(Section::Idle));
    let word = list
        .nodes
        .iter()
        .find_map(|n| match n {
            Node::Text { rect, text, .. } if &**text == "T\u{dc}M G\u{dc}N" => Some(*rect),
            _ => None,
        })
        .expect("word drawn");
    let hit = click(&targets, (word.x + word.w - 8.0, word.y + word.h / 2.0));
    assert_eq!(
        hit,
        Some(SheetAction::Change(SettingChange::Span(RunSpan::Day)))
    );
}

#[test]
fn toggle_flips() {
    let (_, targets) = sheet_build(&facts(), &view(Section::Idle));
    let t = target_for(&targets, &SheetAction::Change(SettingChange::Weather(true)));
    assert_eq!(
        click(&targets, center(t)),
        Some(SheetAction::Change(SettingChange::Weather(true)))
    );
    let mut on = facts();
    on.settings.weather = true;
    let (_, targets) = sheet_build(&on, &view(Section::Idle));
    target_for(
        &targets,
        &SheetAction::Change(SettingChange::Weather(false)),
    );
}

#[test]
fn general_has_a_start_with_windows_row_that_flips() {
    let (list, targets) = sheet_build(&facts(), &view(Section::General));
    let t = target_for(&targets, &SheetAction::Autostart(true));
    assert_eq!(
        click(&targets, center(t)),
        Some(SheetAction::Autostart(true))
    );
    assert!(texts(&list).contains(&"Windows ile ba\u{15f}lat".to_owned()));
    let mut on = facts();
    on.autostart = true;
    let (_, targets) = sheet_build(&on, &view(Section::General));
    target_for(&targets, &SheetAction::Autostart(false));
}

#[test]
fn the_autostart_row_is_in_every_language() {
    for lang in Lang::ALL {
        let (list, targets) = sheet_build(&facts_in(lang), &view(Section::General));
        let row = sheet_text(SheetText::RowAutostart, lang).to_owned();
        assert!(texts(&list).contains(&row), "{lang:?}");
        target_for(&targets, &SheetAction::Autostart(true));
    }
}

#[test]
fn flyout_row_dead_while_osd_off() {
    let mut f = facts();
    let (_, live) = sheet_build(&f, &view(Section::Osd));
    target_for(
        &live,
        &SheetAction::Change(SettingChange::Flyout(FlyoutPref::Hide)),
    );
    f.settings.osd = false;
    let (list, dead) = sheet_build(&f, &view(Section::Osd));
    assert!(
        !dead
            .iter()
            .any(|t| matches!(t.action, SheetAction::Change(SettingChange::Flyout(_))))
    );
    target_for(&dead, &SheetAction::Change(SettingChange::Osd(true)));
    let label = list.nodes.iter().find_map(|n| match n {
        Node::Text { text, ink, .. } if text.starts_with("Windows g\u{f6}stergesini") => Some(*ink),
        _ => None,
    });
    assert_eq!(label, Some(Ink::Faint));
}

#[test]
fn flyout_row_dead_while_the_command_line_sets_it() {
    let mut f = facts();
    f.flyout_locked = true;
    let (list, targets) = sheet_build(&f, &view(Section::Osd));
    assert!(
        !targets
            .iter()
            .any(|t| matches!(t.action, SheetAction::Change(SettingChange::Flyout(_))))
    );
    let label = list.nodes.iter().find_map(|n| match n {
        Node::Text { text, ink, .. } if text.starts_with("Windows g\u{f6}stergesini") => Some(*ink),
        _ => None,
    });
    assert_eq!(label, Some(Ink::Faint));
}

#[test]
fn about_shows_the_build_date_next_to_the_version() {
    let (list, _) = sheet_build(&facts(), &view(Section::About));
    assert!(texts(&list).iter().any(|t| t == "0.4.0  2026-11-20"));
}

#[test]
fn city_field_only_for_city() {
    let mut f = facts();
    let (_, targets) = sheet_build(&f, &view(Section::Idle));
    assert!(!targets.iter().any(|t| t.action == SheetAction::FocusField));
    f.settings.location = LocationMode::City;
    f.settings.city = "Izmir".into();
    let (list, targets) = sheet_build(&f, &view(Section::Idle));
    assert!(targets.iter().any(|t| t.action == SheetAction::FocusField));
    assert!(texts(&list).contains(&"Izmir".to_owned()));
}

#[test]
fn field_shows_the_edit_text_while_focused() {
    let mut f = facts();
    f.settings.location = LocationMode::City;
    f.settings.city = "Izmir".into();
    let mut v = view(Section::Idle);
    v.focus = Some(FocusId::Row(5, 3));
    v.field.text = "Ank".into();
    let (list, _) = sheet_build(&f, &v);
    assert!(texts(&list).contains(&"Ank".to_owned()));
}

#[test]
fn language_choice_wraps_and_picks() {
    let (list, targets) = sheet_build(&facts(), &view(Section::General));
    let pick = |c: LangChoice| SheetAction::Change(SettingChange::Lang(c));
    let auto = target_for(&targets, &pick(LangChoice::Auto)).rect;
    let pl = target_for(&targets, &pick(LangChoice::Code("pl".into()))).rect;
    target_for(&targets, &pick(LangChoice::Code("tr".into())));
    assert!(pl.y > auto.y, "nine words wrap onto a second line");
    assert!(texts(&list).contains(&"T\u{fc}rk\u{e7}e".to_owned()));
    let lang_targets = targets
        .iter()
        .filter(|t| matches!(t.action, SheetAction::Change(SettingChange::Lang(_))))
        .count();
    assert_eq!(lang_targets, 9);
}

#[test]
fn about_open_gives_open_folder() {
    let (list, targets) = sheet_build(&facts(), &view(Section::About));
    target_for(&targets, &SheetAction::OpenFolder);
    assert!(texts(&list).contains(&"0.4.0  2026-11-20".to_owned()));
}

#[test]
fn about_opens_with_the_icon() {
    let (list, _) = sheet_build(&facts(), &view(Section::About));
    let icons: Vec<Rect> = list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Image { rect, art } if *art == ICON_ART => Some(*rect),
            _ => None,
        })
        .collect();
    let expected = Rect {
        x: layout::X,
        y: layout::FIRST_ROW,
        w: ICON_DIP,
        h: ICON_DIP,
    };
    assert_eq!(icons, vec![expected]);
}

#[test]
fn about_names_the_app_and_version() {
    let (list, _) = sheet_build(&facts(), &view(Section::About));
    let text_at = |want: &str| {
        list.nodes.iter().find_map(|n| match n {
            Node::Text {
                rect,
                text,
                style,
                ink,
                ..
            } if &**text == want => Some((*rect, *style, *ink)),
            _ => None,
        })
    };
    let (name, style, _) = text_at("Plith").expect("the app name");
    assert_eq!(style, TextStyle::Hero);
    assert!((name.x - (layout::X + ICON_DIP + 20.0)).abs() < 1e-9);
    let (version, style, ink) = text_at("0.4.0  2026-11-20").expect("version and date");
    assert_eq!((style, ink), (TextStyle::Value, Ink::Muted));
    assert!(version.y > name.y, "the version sits under the name");
}

#[test]
fn about_rows_follow_the_header() {
    let (list, targets) = sheet_build(&facts(), &view(Section::About));
    let top = layout::FIRST_ROW + ICON_DIP + 24.0;
    let first_rule = list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Stroke { points, .. } if points.len() == 2 && points[0].0 >= layout::X => {
                Some(points[0].1)
            }
            _ => None,
        })
        .fold(f64::INFINITY, f64::min);
    assert!(
        (first_rule - top).abs() < 1e-9,
        "first rule at {first_rule}"
    );
    let open = target_for(&targets, &SheetAction::OpenFolder).rect;
    assert!(open.y >= top && open.y + open.h <= top + layout::ROW_SUB);
}

#[test]
fn selected_section_station_is_flow() {
    for (i, s) in Section::ALL.into_iter().enumerate() {
        let (list, _) = sheet_build(&facts(), &view(s));
        let filled: Vec<(f64, Ink)> = list
            .nodes
            .iter()
            .filter_map(|n| match n {
                Node::Station {
                    x,
                    y,
                    mark: StationMark::Filled,
                    ink,
                } if (*x - 32.0).abs() < 1e-9 => Some((*y, *ink)),
                _ => None,
            })
            .collect();
        #[allow(clippy::cast_precision_loss)] // five sections
        let y = 58.0 + i as f64 * 32.0 + 14.0;
        assert_eq!(filled, vec![(y, Ink::Flow)], "{s:?}");
    }
}

#[test]
fn save_error_replaces_the_line() {
    let mut f = facts();
    let line = crate::lang::sheet::sheet_text(crate::lang::sheet::SheetText::LineIdle, Lang::Tr);
    let (list, _) = sheet_build(&f, &view(Section::Idle));
    assert!(texts(&list).iter().any(|t| t == line));
    f.save_failed = true;
    let (list, _) = sheet_build(&f, &view(Section::Idle));
    assert!(!texts(&list).iter().any(|t| t == line));
    assert!(
        texts(&list)
            .iter()
            .any(|t| t.starts_with("Ayarlar kaydedilemedi"))
    );
}

#[test]
fn focus_and_hover_outline_a_target() {
    let base = sheet_build(&facts(), &view(Section::Idle)).0.nodes.len();
    let mut v = view(Section::Idle);
    v.focus = Some(FocusId::Row(1, 0));
    assert_eq!(sheet_build(&facts(), &v).0.nodes.len(), base + 1);
    v.hover = Some(FocusId::Row(2, 0));
    assert_eq!(sheet_build(&facts(), &v).0.nodes.len(), base + 2);
}

#[test]
fn flow_only_on_the_selected_station() {
    // ADR 0020: the selected section's sidebar station is the one flow-coloured mark.
    let mut f = facts();
    f.settings.head = HeadSlot::Playing;
    f.settings.location = LocationMode::City;
    for s in Section::ALL {
        let (list, _) = sheet_build(&f, &view(s));
        for n in &list.nodes {
            if let Node::Station { x, .. } = n
                && (*x - 32.0).abs() < 1e-9
            {
                continue;
            }
            let ink = match n {
                Node::Fill { ink, .. }
                | Node::Stroke { ink, .. }
                | Node::Polygon { ink, .. }
                | Node::Station { ink, .. }
                | Node::Text { ink, .. } => Some(*ink),
                _ => None,
            };
            assert_ne!(ink, Some(Ink::Flow), "{s:?}");
        }
    }
}

fn inside(x: f64, y: f64) -> bool {
    (0.0..=SHEET_W).contains(&x) && (0.0..=SHEET_H).contains(&y)
}

#[test]
fn fits_840_by_640() {
    let mut f = facts_in(Lang::De);
    f.settings.location = LocationMode::City;
    f.save_failed = true;
    for s in Section::ALL {
        let (list, targets) = sheet_build(&f, &view(s));
        assert_eq!(list.size, (SHEET_W, SHEET_H));
        let rects = list.nodes.iter().filter_map(|n| match n {
            Node::Fill { rect, .. } | Node::Text { rect, .. } => Some(*rect),
            Node::Inset { x, y, scale, list } => Some(Rect {
                x: *x,
                y: *y,
                w: list.size.0 * scale,
                h: list.size.1 * scale,
            }),
            _ => None,
        });
        for r in rects.chain(targets.iter().map(|t| t.rect)) {
            assert!(
                inside(r.x, r.y) && inside(r.x + r.w, r.y + r.h),
                "{s:?} {r:?}"
            );
        }
        for n in &list.nodes {
            match n {
                Node::Stroke { points, .. } | Node::Polygon { points, .. } => {
                    assert!(points.iter().all(|&(x, y)| inside(x, y)), "{s:?}");
                }
                Node::Station { x, y, .. } => assert!(inside(*x, *y), "{s:?}"),
                _ => {}
            }
        }
    }
}

#[test]
fn hit_has_no_slack_and_takes_the_first() {
    let r = |x| Rect {
        x,
        y: 0.0,
        w: 10.0,
        h: 10.0,
    };
    let t = |x, action| SheetTarget {
        rect: r(x),
        id: FocusId::Row(0, 0),
        action,
    };
    let targets = [
        t(0.0, SheetAction::FocusField),
        t(5.0, SheetAction::OpenFolder),
    ];
    assert_eq!(
        sheet_hit(&targets, 7.0, 5.0).unwrap().action,
        SheetAction::FocusField
    );
    assert_eq!(
        sheet_hit(&targets, 12.0, 5.0).unwrap().action,
        SheetAction::OpenFolder
    );
    assert!(sheet_hit(&targets, 15.0, 5.0).is_none());
    assert!(sheet_hit(&targets, 5.0, 10.0).is_none());
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

#[test]
fn wrapped_language_line_clears_the_sub_line() {
    for lang in [Lang::De, Lang::Tr] {
        let (list, targets) = sheet_build(&facts_in(lang), &view(Section::General));
        let sub = crate::lang::sheet::sheet_text(crate::lang::sheet::SheetText::SubLanguage, lang);
        let sub_rect = list
            .nodes
            .iter()
            .find_map(|n| match n {
                Node::Text { rect, text, .. } if &**text == sub => Some(*rect),
                _ => None,
            })
            .expect("sub line drawn");
        let lang_rects = targets
            .iter()
            .filter(|t| matches!(t.action, SheetAction::Change(SettingChange::Lang(_))))
            .map(|t| t.rect)
            .collect::<Vec<_>>();
        let first_y = lang_rects.iter().map(|r| r.y).fold(f64::MAX, f64::min);
        assert!(lang_rects.iter().any(|r| r.y > first_y), "{lang:?} wraps");
        for r in lang_rects.iter().filter(|r| r.y > first_y) {
            assert!(!overlaps(*r, sub_rect), "{lang:?} {r:?} over {sub_rect:?}");
        }
    }
}

fn filled_stations(list: &DisplayList) -> usize {
    list.nodes
        .iter()
        .filter(|n| matches!(n, Node::Station { x, mark: StationMark::Filled, .. } if *x > 700.0))
        .count()
}

#[test]
fn toggle_off_has_no_filled_station() {
    let mut f = facts();
    f.settings.osd = false;
    let (off, _) = sheet_build(&f, &view(Section::Osd));
    assert_eq!(filled_stations(&off), 0);
    f.settings.osd = true;
    let (on, _) = sheet_build(&f, &view(Section::Osd));
    assert_eq!(filled_stations(&on), 1);
}

#[test]
fn heading_is_twenty_dip() {
    let (list, _) = sheet_build(&facts(), &view(Section::Idle));
    assert!(list.nodes.iter().any(|n| matches!(
        n,
        Node::Text { style: TextStyle::Heading, text, .. } if &**text == "Bo\u{15f}ta"
    )));
    assert!((crate::draw::style_spec(TextStyle::Heading).size_dip - 20.0).abs() < f32::EPSILON);
}

/// The radius and ink of every fill whose rect equals `r`.
fn fills_at(list: &DisplayList, r: Rect) -> Vec<(f64, Ink)> {
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    list.nodes
        .iter()
        .filter_map(|n| match n {
            Node::Fill { rect, radius, ink }
                if near(rect.x, r.x)
                    && near(rect.y, r.y)
                    && near(rect.w, r.w)
                    && near(rect.h, r.h) =>
            {
                Some((*radius, *ink))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn preview_pills_are_raised_and_outlined() {
    let (list, _) = sheet_build(&facts(), &view(Section::Idle));
    let insets: Vec<(f64, f64, f64)> = list
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Inset { x, y, scale, .. } => Some((*x, *y, *scale)),
            _ => None,
        })
        .collect();
    assert_eq!(insets.len(), 6);
    for (i, (x, y, scale)) in insets.into_iter().enumerate() {
        let shape = crate::pages::idle_pill::rest_shape(LOOKS[i]);
        let (w, h) = (shape.width * scale, shape.height * scale);
        let outer = fills_at(&list, Rect { x, y, w, h });
        let inner = fills_at(
            &list,
            Rect {
                x: x + 1.0,
                y: y + 1.0,
                w: w - 2.0,
                h: h - 2.0,
            },
        );
        if LOOKS[i] == IdleLook::Hidden {
            assert!(outer.is_empty() && inner.is_empty(), "hidden has no pill");
            continue;
        }
        let inks = |f: &[(f64, Ink)]| f.iter().map(|x| x.1).collect::<Vec<_>>();
        assert_eq!(inks(&outer), [Ink::Border], "preview {i}");
        assert_eq!(inks(&inner), [Ink::FloorHigh], "preview {i}");
        assert!((outer[0].0 - shape.drawn_radius() * scale).abs() < 1e-9);
    }
}
