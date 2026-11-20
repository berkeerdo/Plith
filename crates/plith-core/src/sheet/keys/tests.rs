use super::*;
use crate::draw::{Ink, Node};
use crate::settings::change::SettingChange;
use crate::settings::{IdleLook, LocationMode};
use crate::sheet::tests::{facts, view};
use crate::sheet::{FocusId, Section, SheetAction, sheet_build};

const FIELD: FocusId = FocusId::Row(5, 3);

fn city_facts() -> SheetFacts {
    let mut f = facts();
    f.settings.location = LocationMode::City;
    f.settings.city = "Izmir".into();
    f
}

fn focused_view(f: &SheetFacts) -> SheetView {
    let mut v = view(Section::Idle);
    v.focus = Some(FIELD);
    focus_field(f, &mut v);
    v
}

fn city(text: &str) -> KeyOutcome {
    KeyOutcome::Act(SheetAction::Change(SettingChange::City(text.into())))
}

#[test]
fn tab_walks_in_order() {
    let f = facts();
    let mut v = view(Section::Idle);
    let order: Vec<FocusId> = sheet_build(&f, &v).1.iter().map(|t| t.id).collect();
    assert!(order.len() > 5);
    assert_eq!(
        order[0],
        FocusId::Section(Section::General),
        "sidebar first"
    );
    for id in &order {
        assert_eq!(on_key(&f, &mut v, SheetKey::Tab), KeyOutcome::Redraw);
        assert_eq!(v.focus, Some(*id));
    }
    on_key(&f, &mut v, SheetKey::Tab);
    assert_eq!(v.focus, Some(order[0]), "wraps");
    on_key(&f, &mut v, SheetKey::BackTab);
    assert_eq!(v.focus, order.last().copied());
}

#[test]
fn arrows_pick_in_group() {
    let f = facts();
    let mut v = view(Section::Idle);
    let targets = sheet_build(&f, &v).1;
    let looks: Vec<_> = targets
        .iter()
        .filter(|t| matches!(t.action, SheetAction::Change(SettingChange::Look(_))))
        .collect();
    assert!(looks.len() >= 3);
    v.focus = Some(looks[1].id);
    let out = on_key(&f, &mut v, SheetKey::Right);
    assert_eq!(out, KeyOutcome::Act(looks[2].action.clone()));
    assert_eq!(v.focus, Some(looks[2].id));
    let out = on_key(&f, &mut v, SheetKey::Left);
    assert_eq!(out, KeyOutcome::Act(looks[1].action.clone()));
    v.focus = Some(looks[0].id);
    assert_eq!(on_key(&f, &mut v, SheetKey::Left), KeyOutcome::Nothing);
    assert_eq!(v.focus, Some(looks[0].id), "ends do not wrap");
    assert!(matches!(
        looks[0].action,
        SheetAction::Change(SettingChange::Look(IdleLook::Karot))
    ));
}

#[test]
fn space_and_enter_act_on_focus() {
    let f = facts();
    let mut v = view(Section::Idle);
    let targets = sheet_build(&f, &v).1;
    let t = targets
        .iter()
        .find(|t| matches!(t.action, SheetAction::Change(_)))
        .expect("a change target");
    v.focus = Some(t.id);
    let want = KeyOutcome::Act(t.action.clone());
    assert_eq!(on_key(&f, &mut v, SheetKey::Space), want);
    assert_eq!(on_key(&f, &mut v, SheetKey::Enter), want);
    v.focus = None;
    assert_eq!(on_key(&f, &mut v, SheetKey::Enter), KeyOutcome::Nothing);
}

#[test]
fn escape_closes_outside_field() {
    let f = facts();
    let mut v = view(Section::Idle);
    assert_eq!(on_key(&f, &mut v, SheetKey::Escape), KeyOutcome::Close);
}

#[test]
fn enter_in_field_commits_once() {
    let f = city_facts();
    let mut v = focused_view(&f);
    assert_eq!(v.field.text, "Izmir");
    assert_eq!(v.field.caret, 5);
    for c in "XY".chars() {
        assert_eq!(on_key(&f, &mut v, SheetKey::Char(c)), KeyOutcome::Redraw);
    }
    assert_eq!(on_key(&f, &mut v, SheetKey::Enter), city("IzmirXY"));
    assert_eq!(
        on_key(&f, &mut v, SheetKey::Enter),
        KeyOutcome::Redraw,
        "nothing new to commit"
    );
}

#[test]
fn escape_in_field_restores_and_leaves() {
    let f = city_facts();
    let mut v = focused_view(&f);
    on_key(&f, &mut v, SheetKey::Char('Z'));
    assert_eq!(on_key(&f, &mut v, SheetKey::Escape), KeyOutcome::Redraw);
    assert_eq!(v.field.text, "Izmir");
    assert_eq!(v.focus, None);
}

#[test]
fn tab_out_of_field_commits_and_moves() {
    let f = city_facts();
    let mut v = focused_view(&f);
    on_key(&f, &mut v, SheetKey::Char('!'));
    assert_eq!(on_key(&f, &mut v, SheetKey::Tab), city("Izmir!"));
    assert_ne!(v.focus, Some(FIELD));
    assert!(v.focus.is_some());
}

#[test]
fn commit_field_only_when_changed() {
    let f = city_facts();
    let mut v = focused_view(&f);
    assert_eq!(commit_field(&mut v), None);
    assert_eq!(v.focus, None);
    let mut v = focused_view(&f);
    on_key(&f, &mut v, SheetKey::Char('Q'));
    assert_eq!(commit_field(&mut v), Some("IzmirQ".into()));
    assert_eq!(commit_field(&mut v), None, "not focused any more");
}

#[test]
fn copy_in_field_returns_text() {
    let f = city_facts();
    let mut v = focused_view(&f);
    assert_eq!(on_key(&f, &mut v, SheetKey::SelectAll), KeyOutcome::Redraw);
    assert_eq!(
        on_key(&f, &mut v, SheetKey::Copy),
        KeyOutcome::Copy("Izmir".into())
    );
}

#[test]
fn caret_and_selection_are_drawn() {
    let f = city_facts();
    let mut v = focused_view(&f);
    let count = |v: &SheetView, ink: Ink, w: f64| {
        sheet_build(&f, v)
            .0
            .nodes
            .iter()
            .filter(|n| {
                matches!(n, Node::Fill { rect, ink: i, .. }
                    if *i == ink && (rect.w - w).abs() < 1e-9)
            })
            .count()
    };
    v.caret_x = 30.0;
    v.text_w = 38.0;
    v.caret_on = false;
    assert_eq!(count(&v, Ink::Ink, 1.0), 0, "caret off");
    v.caret_on = true;
    assert_eq!(count(&v, Ink::Ink, 1.0), 1);
    assert_eq!(count(&v, Ink::Track, 38.0), 0);
    v.field.all = true;
    assert_eq!(count(&v, Ink::Track, 38.0), 1);
    v.focus = None;
    assert_eq!(count(&v, Ink::Ink, 1.0), 0, "no caret without focus");
}
