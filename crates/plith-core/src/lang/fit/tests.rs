use super::*;

/// Strings no page draws yet: the look names wait for the settings page, and the two welcome
/// lines for nothing played and a paused track are not built.
const NOT_DRAWN: [Text; 8] = [
    Text::LookKarot,
    Text::LookClock,
    Text::LookPlaying,
    Text::LookSeal,
    Text::LookPlain,
    Text::LookHidden,
    Text::NothingPlayed,
    Text::Paused,
];

#[test]
fn every_drawn_string_has_a_place() {
    for lang in Lang::ALL {
        let all = places(lang);
        for t in Text::ALL {
            if NOT_DRAWN.contains(&t) {
                continue;
            }
            let s = text(t, lang);
            assert!(
                all.iter().any(|p| p.text.contains(s)),
                "{lang:?} {t:?} has no place in the fit list"
            );
        }
    }
}

#[test]
fn places_keep_one_name_set_in_every_language() {
    let names = |lang| places(lang).into_iter().map(|p| p.name).collect::<Vec<_>>();
    let tr = names(Lang::Tr);
    for lang in Lang::ALL {
        assert_eq!(names(lang), tr, "{lang:?}");
    }
}

#[test]
fn budgets_are_positive() {
    for p in places(Lang::En) {
        if let Budget::Dip(w) = p.budget {
            assert!(w > 0.0, "{} has no room", p.name);
        }
    }
}
