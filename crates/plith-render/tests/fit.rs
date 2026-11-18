//! Own copy against the bundled fonts: every string fits its place and has its glyphs.
//! DirectWrite only, no GPU, CI-safe.

use plith_core::draw::Trim;
use plith_core::lang::Lang;
use plith_core::lang::fit::{Budget, glyph_strings, places};
use plith_render::TextSystem;

/// Wider than any line, so `measure` never trims.
const UNBOUNDED: f32 = 100_000.0;
/// Sub-pixel slack for the layout's rounding.
const EPSILON: f64 = 0.01;

fn system() -> TextSystem {
    let mut text = TextSystem::new().expect("text system");
    assert!(text.uses_bundled_fonts(), "notes: {:?}", text.take_notes());
    text
}

fn width(text: &mut TextSystem, s: &str, style: plith_core::draw::TextStyle) -> f64 {
    f64::from(
        text.measure(s, style, UNBOUNDED, Trim::End)
            .expect("measure"),
    )
}

#[test]
fn every_own_string_fits_its_place() {
    let mut text = system();
    let turkish = places(Lang::Tr);
    let mut failures = Vec::new();
    let mut widest = std::collections::BTreeMap::<String, (Lang, f64, f64)>::new();
    for lang in Lang::ALL {
        for (i, place) in places(lang).into_iter().enumerate() {
            let w = width(&mut text, &place.text, place.style);
            let budget = match place.budget {
                Budget::Dip(dip) => dip,
                Budget::LikeTurkish => {
                    let tr = &turkish[i];
                    assert_eq!(tr.name, place.name);
                    width(&mut text, &tr.text, tr.style)
                }
            };
            let entry = widest
                .entry(place.name.clone())
                .or_insert((lang, w, budget));
            if w > entry.1 {
                *entry = (lang, w, budget);
            }
            if w > budget + EPSILON {
                failures.push(format!(
                    "{lang:?} {:?} ({:?}): {w:.1} > {budget:.1} DIP, {:?}",
                    place.name, place.style, place.text
                ));
            }
        }
    }
    if std::env::var_os("PLITH_FIT_TABLE").is_some() {
        for (name, (lang, w, budget)) in &widest {
            println!("{name}\t{budget:.1}\t{lang:?}\t{w:.1}");
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn every_own_string_has_glyphs() {
    let text = system();
    let mut failures = Vec::new();
    for lang in Lang::ALL {
        for (s, style) in glyph_strings(lang) {
            let missing = text
                .missing_glyphs(&s, style)
                .expect("glyph lookup")
                .expect("bundled fonts");
            if !missing.is_empty() {
                failures.push(format!("{lang:?} {style:?} {s:?} lacks {missing:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
