use crate::draw::{DisplayList, Node};
use crate::lang::Lang;
use crate::settings::change::SettingChange;
use crate::sheet::tests::{facts, view};
use crate::sheet::{
    FocusId, KeyOutcome, Section, SheetAction, SheetKey, SheetTarget, on_key, sheet_build,
    sheet_hit,
};
use crate::theme::registry::ThemeId;

fn texts(list: &DisplayList) -> Vec<String> {
    list.nodes
        .iter()
        .filter_map(|n| match n {
            Node::Text { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

fn theme_targets(targets: &[SheetTarget]) -> Vec<&SheetTarget> {
    targets
        .iter()
        .filter(|t| matches!(t.action, SheetAction::Change(SettingChange::Theme(_))))
        .collect()
}

fn id(text: &str) -> ThemeId {
    ThemeId::parse(text).unwrap()
}

/// The ids of the facts' cards, in listing order.
fn ids() -> Vec<ThemeId> {
    facts().themes.into_iter().map(|card| card.id).collect()
}

fn pick(theme: ThemeId) -> SheetAction {
    SheetAction::Change(SettingChange::Theme(theme))
}

#[test]
fn the_look_section_sits_between_general_and_idle() {
    assert_eq!(
        Section::ALL,
        [
            Section::General,
            Section::Look,
            Section::Idle,
            Section::Osd,
            Section::About
        ]
    );
    let (_, targets) = sheet_build(&facts(), &view(Section::Look));
    let y = |s: Section| {
        targets
            .iter()
            .find(|t| t.action == SheetAction::Section(s))
            .unwrap()
            .rect
            .y
    };
    assert!(y(Section::General) < y(Section::Look));
    assert!(y(Section::Look) < y(Section::Idle));
}

#[test]
fn the_theme_row_lists_the_pack_names_of_the_facts() {
    let (list, targets) = sheet_build(&facts(), &view(Section::Look));
    let all = texts(&list);
    let mut at = 0;
    for label in ["G\u{f6}r\u{fc}n\u{fc}m", "Tema"] {
        let found = all[at..].iter().position(|t| t == label);
        assert!(found.is_some(), "{label} missing or out of order");
        at += found.unwrap() + 1;
    }
    for name in ["GECE", "HALKA", "F\u{130}RN", "KABLO"] {
        assert!(all.iter().any(|t| t == name), "{name}");
    }
    let groups = theme_targets(&targets);
    assert_eq!(groups.len(), 4);
    for (target, theme) in groups.iter().zip(ids()) {
        assert_eq!(target.action, pick(theme));
    }
}

#[test]
fn clicking_a_theme_word_picks_that_theme() {
    let (_, targets) = sheet_build(&facts(), &view(Section::Look));
    for theme in ids() {
        let t = targets
            .iter()
            .find(|t| t.action == pick(theme.clone()))
            .unwrap();
        let (x, y) = (t.rect.x + t.rect.w / 2.0, t.rect.y + t.rect.h / 2.0);
        assert_eq!(sheet_hit(&targets, x, y).unwrap().action, pick(theme));
    }
}

#[test]
fn only_the_current_theme_is_underlined() {
    let mut f = facts();
    for theme in ids() {
        f.settings.theme = theme.clone();
        let (list, targets) = sheet_build(&f, &view(Section::Look));
        let at = targets
            .iter()
            .find(|t| t.action == pick(theme.clone()))
            .unwrap();
        let underlines: Vec<f64> = list
            .nodes
            .iter()
            .filter_map(|n| match n {
                Node::Stroke { points, .. }
                    if points.len() == 2
                        && (points[0].1 - points[1].1).abs() < f64::EPSILON
                        && points[0].0 > 184.0
                        && points[0].1 > at.rect.y
                        && points[0].1 < at.rect.y + at.rect.h + 4.0 =>
                {
                    Some(points[0].0)
                }
                _ => None,
            })
            .collect();
        assert_eq!(underlines.len(), 1, "{theme:?}");
        assert!(underlines[0] > at.rect.x && underlines[0] < at.rect.x + at.rect.w);
    }
}

#[test]
fn arrows_walk_the_choice_and_tab_visits_each_word() {
    let f = facts();
    let mut v = view(Section::Look);
    v.focus = Some(FocusId::Row(0, 0));
    assert_eq!(
        on_key(&f, &mut v, SheetKey::Right),
        KeyOutcome::Act(pick(id("halka")))
    );
    assert_eq!(v.focus, Some(FocusId::Row(0, 1)));
    assert_eq!(
        on_key(&f, &mut v, SheetKey::Left),
        KeyOutcome::Act(pick(id("gece")))
    );
    assert_eq!(on_key(&f, &mut v, SheetKey::Left), KeyOutcome::Nothing);
    let mut seen = Vec::new();
    let mut v = view(Section::Look);
    for _ in 0..12 {
        on_key(&f, &mut v, SheetKey::Tab);
        if let Some(id @ FocusId::Row(0, _)) = v.focus {
            seen.push(id);
        }
    }
    seen.dedup();
    assert_eq!(
        &seen[..4],
        [
            FocusId::Row(0, 0),
            FocusId::Row(0, 1),
            FocusId::Row(0, 2),
            FocusId::Row(0, 3)
        ]
    );
}

#[test]
fn the_section_reads_in_every_language() {
    let mut f = facts();
    for lang in Lang::ALL {
        f.lang = lang;
        let (list, _) = sheet_build(&f, &view(Section::Look));
        let all = texts(&list);
        assert!(all.iter().any(|t| t == "KABLO"), "{lang:?}");
        assert!(all.len() > 8, "{lang:?}");
    }
}
