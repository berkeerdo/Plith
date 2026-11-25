//! The update rows of the About and General sections.

use super::tests::{facts, facts_in, target_for, texts, view};
use super::*;
use crate::lang::sheet::{SheetText, fill, sheet_text};
use crate::setup::Version;
use crate::update::UpdateRow;

fn about_with(row: UpdateRow) -> (Vec<String>, Vec<SheetTarget>) {
    let mut f = facts();
    f.update = row;
    let (list, targets) = sheet_build(&f, &view(Section::About));
    (texts(&list), targets)
}

#[test]
fn about_shows_update_row_per_state() {
    let ready = Version::new(0, 5, 0);
    let cases = [
        (
            UpdateRow::Current,
            "G\u{dc}NCEL",
            Some(SheetAction::UpdateCheck),
        ),
        (
            UpdateRow::Ready(ready),
            "0.5.0 HAZIR",
            Some(SheetAction::UpdateInstall),
        ),
        (UpdateRow::Checking, "DENETLEN\u{130}YOR", None),
        (
            UpdateRow::Downloading(42),
            "\u{130}ND\u{130}R\u{130}L\u{130}YOR 42%",
            None,
        ),
        (
            UpdateRow::BadSignature,
            "\u{130}MZA GE\u{c7}ERS\u{130}Z",
            Some(SheetAction::UpdateCheck),
        ),
        (
            UpdateRow::Failed,
            "DENETLENEMED\u{130}",
            Some(SheetAction::UpdateCheck),
        ),
    ];
    for (row, state, button) in cases {
        let (shown, targets) = about_with(row);
        assert!(shown.contains(&state.to_owned()), "{row:?} shows {state}");
        assert!(
            shown.contains(&"G\u{fc}ncelleme".to_owned()),
            "{row:?} label"
        );
        let has = |a: &SheetAction| targets.iter().any(|t| &t.action == a);
        for action in [SheetAction::UpdateCheck, SheetAction::UpdateInstall] {
            assert_eq!(
                has(&action),
                button.as_ref() == Some(&action),
                "{row:?} {action:?}"
            );
        }
        let word = match button {
            Some(SheetAction::UpdateInstall) => "KUR",
            Some(_) => "DENETLE",
            None => continue,
        };
        assert!(shown.contains(&word.to_owned()), "{row:?} button {word}");
    }
}

#[test]
fn an_unchecked_row_offers_a_check_and_claims_nothing() {
    let (shown, targets) = about_with(UpdateRow::Unknown);
    assert!(shown.contains(&"DENETLE".to_owned()));
    assert!(targets.iter().any(|t| t.action == SheetAction::UpdateCheck));
    assert!(
        !targets
            .iter()
            .any(|t| t.action == SheetAction::UpdateInstall)
    );
    for claim in ["G\u{dc}NCEL", "DENETLENEMED\u{130}"] {
        assert!(!shown.contains(&claim.to_owned()), "claims {claim}");
    }
}

#[test]
fn update_button_sits_inside_the_content_column() {
    let (_, targets) = about_with(UpdateRow::Ready(Version::new(1, 0, 0)));
    let install = target_for(&targets, &SheetAction::UpdateInstall).rect;
    assert!(install.x >= layout::X && install.x + install.w <= layout::R + 1e-9);
    let open = target_for(&targets, &SheetAction::OpenFolder).rect;
    assert!(install.y > open.y, "the update row follows the file row");
}

#[test]
fn update_state_text_is_per_language() {
    for lang in Lang::ALL {
        let mut f = facts_in(lang);
        f.update = UpdateRow::Downloading(7);
        let (list, _) = sheet_build(&f, &view(Section::About));
        let want = fill(
            sheet_text(SheetText::StateUpdateDownloading, lang),
            "percent",
            "7",
        );
        assert!(texts(&list).contains(&want), "{lang:?}: {want}");
    }
}

#[test]
fn general_update_toggle_flips_setting() {
    let mut f = facts();
    assert!(f.settings.update_check, "on by default");
    let (list, targets) = sheet_build(&f, &view(Section::General));
    target_for(
        &targets,
        &SheetAction::Change(SettingChange::UpdateCheck(false)),
    );
    assert!(texts(&list).contains(&"G\u{fc}ncellemeleri denetle".to_owned()));
    f.settings.update_check = false;
    let (_, targets) = sheet_build(&f, &view(Section::General));
    target_for(
        &targets,
        &SheetAction::Change(SettingChange::UpdateCheck(true)),
    );
}

#[test]
fn general_update_toggle_has_its_own_row_index() {
    let (_, targets) = sheet_build(&facts(), &view(Section::General));
    let ids: Vec<FocusId> = targets.iter().map(|t| t.id).collect();
    let toggle = target_for(
        &targets,
        &SheetAction::Change(SettingChange::UpdateCheck(false)),
    );
    assert_eq!(toggle.id, FocusId::Row(3, 0));
    assert_eq!(ids.iter().filter(|id| **id == toggle.id).count(), 1);
}

#[test]
fn data_line_names_github_in_every_language() {
    for lang in Lang::ALL {
        assert!(
            sheet_text(SheetText::SubData, lang).contains("GitHub"),
            "{lang:?}"
        );
    }
}
