use super::*;

/// Strings with no place of their own: `Paused` is appended to a user-content artist that the
/// layout trims anyway.
const NOT_DRAWN: [Text; 1] = [Text::Paused];

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
