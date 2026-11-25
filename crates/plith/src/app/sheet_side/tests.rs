use super::*;
use plith_core::draw::Rect;
use plith_core::lang::Lang;
use plith_core::settings::{LocationMode, Settings};
use plith_core::sheet::{FocusId, Section, SheetKey, sheet_build};
use std::sync::Arc;

fn facts(settings: Settings) -> SheetFacts {
    SheetFacts {
        settings,
        lang: Lang::En,
        previews: std::array::from_fn(|_| Arc::new(blank())),
        version: "0.0.0",
        build_date: "2026-11-20",
        flyout_locked: false,
        settings_path: String::from("C:\\x\\settings.ini"),
        save_failed: false,
        autostart: false,
        update: plith_core::update::UpdateRow::Current,
    }
}

fn center(r: Rect) -> (f64, f64) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn click(t: &SheetTarget) -> SheetEvent {
    let (x, y) = center(t.rect);
    SheetEvent::Click(x, y)
}

fn first_change(targets: &[SheetTarget]) -> (SheetTarget, SettingChange) {
    targets
        .iter()
        .find_map(|t| match &t.action {
            SheetAction::Change(change) => Some((t.clone(), change.clone())),
            _ => None,
        })
        .unwrap()
}

#[test]
fn sheet_events_route() {
    let facts = facts(Settings::default());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let (target, change) = first_change(&targets);
    let steps = route(click(&target), &facts, &mut view, &targets);
    assert_eq!(steps, vec![Step::Change(change)]);
    let steps = route(
        SheetEvent::Key(SheetKey::Escape),
        &facts,
        &mut view,
        &targets,
    );
    assert_eq!(steps, vec![Step::Close]);
}

#[test]
fn closed_closes_and_resized_resizes() {
    let facts = facts(Settings::default());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    assert_eq!(
        route(SheetEvent::Closed, &facts, &mut view, &targets),
        vec![Step::Close]
    );
    let resized = SheetEvent::Resized {
        width_px: 1680,
        height_px: 1280,
        scale: 2.0,
    };
    assert_eq!(
        route(resized, &facts, &mut view, &targets),
        vec![Step::Resize(1680, 1280, 2.0)]
    );
}

#[test]
fn a_section_click_shows_the_section() {
    let facts = facts(Settings::default());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let about = targets
        .iter()
        .find(|t| t.action == SheetAction::Section(Section::About))
        .unwrap();
    let steps = route(click(about), &facts, &mut view, &targets);
    assert_eq!(steps, vec![Step::Redraw]);
    assert_eq!(view.section, Section::About);
}

#[test]
fn hover_redraws_only_when_it_moves_to_another_control() {
    let facts = facts(Settings::default());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let (target, _) = first_change(&targets);
    let (x, y) = center(target.rect);
    let moved = route(SheetEvent::Move(x, y), &facts, &mut view, &targets);
    assert_eq!(moved, vec![Step::Redraw]);
    assert_eq!(view.hover, Some(target.id));
    let again = route(SheetEvent::Move(x + 1.0, y), &facts, &mut view, &targets);
    assert!(again.is_empty());
    let left = route(SheetEvent::Leave, &facts, &mut view, &targets);
    assert_eq!(left, vec![Step::Redraw]);
    assert_eq!(view.hover, None);
}

fn city_settings() -> Settings {
    Settings {
        weather: true,
        location: LocationMode::City,
        city: String::from("Ankara"),
        ..Settings::default()
    }
}

fn field_of(targets: &[SheetTarget]) -> SheetTarget {
    targets
        .iter()
        .find(|t| t.action == SheetAction::FocusField)
        .unwrap()
        .clone()
}

#[test]
fn a_click_elsewhere_commits_the_field_first() {
    let facts = facts(city_settings());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let field = field_of(&targets);
    assert_eq!(
        route(click(&field), &facts, &mut view, &targets),
        vec![Step::Redraw]
    );
    assert_eq!(view.focus, Some(field.id));
    assert_eq!(view.field.text, "Ankara");
    let key = SheetEvent::Key(SheetKey::Backspace);
    assert_eq!(route(key, &facts, &mut view, &targets), vec![Step::Redraw]);
    let (target, change) = first_change(&targets);
    let steps = route(click(&target), &facts, &mut view, &targets);
    assert_eq!(
        steps,
        vec![
            Step::Change(SettingChange::City(String::from("Ankar"))),
            Step::Change(change)
        ]
    );
    assert_eq!(view.focus, None);
}

#[test]
fn a_second_click_on_the_field_keeps_the_edit() {
    let facts = facts(city_settings());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let field = field_of(&targets);
    let _ = route(click(&field), &facts, &mut view, &targets);
    let _ = route(
        SheetEvent::Key(SheetKey::Char('x')),
        &facts,
        &mut view,
        &targets,
    );
    assert!(route(click(&field), &facts, &mut view, &targets).is_empty());
    assert_eq!(view.field.text, "Ankarax");
}

#[test]
fn copy_and_folder_reach_the_app() {
    let facts = facts(city_settings());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let field = field_of(&targets);
    let _ = route(click(&field), &facts, &mut view, &targets);
    let _ = route(
        SheetEvent::Key(SheetKey::SelectAll),
        &facts,
        &mut view,
        &targets,
    );
    let copied = route(SheetEvent::Key(SheetKey::Copy), &facts, &mut view, &targets);
    assert_eq!(copied, vec![Step::Copy(String::from("Ankara"))]);

    let mut about = SheetView {
        section: Section::About,
        ..SheetView::default()
    };
    let targets = sheet_build(&facts, &about).1;
    let open = targets
        .iter()
        .find(|t| t.action == SheetAction::OpenFolder)
        .unwrap();
    assert_eq!(
        route(click(open), &facts, &mut about, &targets),
        vec![Step::OpenFolder]
    );
}

#[test]
fn closing_commits_a_pending_city() {
    let facts = facts(city_settings());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    let _ = route(click(&field_of(&targets)), &facts, &mut view, &targets);
    let _ = route(
        SheetEvent::Key(SheetKey::Char('y')),
        &facts,
        &mut view,
        &targets,
    );
    assert_eq!(
        route(SheetEvent::Closed, &facts, &mut view, &targets),
        vec![
            Step::Change(SettingChange::City(String::from("Ankaray"))),
            Step::Close
        ]
    );
}

#[test]
fn the_caret_wants_a_timer_only_while_the_field_has_focus() {
    let facts = facts(city_settings());
    let mut view = SheetView::default();
    let targets = sheet_build(&facts, &view).1;
    assert!(!field_focused(&view, &targets));
    let field = field_of(&targets);
    let _ = route(click(&field), &facts, &mut view, &targets);
    assert!(field_focused(&view, &targets));
    view.focus = Some(FocusId::Section(Section::Idle));
    assert!(!field_focused(&view, &targets));
}

#[test]
fn the_about_update_buttons_route_to_the_update_steps() {
    use plith_core::setup::Version;
    use plith_core::update::UpdateRow;
    let cases = [
        (
            UpdateRow::Current,
            SheetAction::UpdateCheck,
            Step::UpdateCheck,
        ),
        (
            UpdateRow::Ready(Version::new(0, 5, 0)),
            SheetAction::UpdateInstall,
            Step::UpdateInstall,
        ),
    ];
    for (row, action, step) in cases {
        let mut facts = facts(Settings::default());
        facts.update = row;
        let mut view = SheetView {
            section: Section::About,
            ..SheetView::default()
        };
        let targets = sheet_build(&facts, &view).1;
        let button = targets.iter().find(|t| t.action == action).unwrap();
        let steps = route(click(button), &facts, &mut view, &targets);
        assert_eq!(steps, vec![step]);
    }
}
