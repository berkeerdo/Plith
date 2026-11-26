use super::*;

fn rgb(text: &str) -> Rgb {
    Rgb::parse(text).unwrap()
}

fn flat(floors: &[&str]) -> Backdrop {
    Backdrop::Flat {
        lums: floors.iter().map(|f| rgb(f).luminance()).collect(),
    }
}

fn gece() -> Backdrop {
    flat(&["#0B0B0A", "#151412"])
}

fn hue_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

#[test]
fn passing_pick_is_unchanged() {
    let solved = solve_accent(rgb("#CAFF33"), &gece(), rgb("#FFFFFF"));
    assert_eq!(solved.colour, rgb("#CAFF33"));
    assert_eq!(solved.shift, Shift::Kept);
}

#[test]
fn dark_pick_on_dark_floor_moves_lighter() {
    let pick = rgb("#1C2A5A");
    let backdrop = gece();
    let solved = solve_accent(pick, &backdrop, rgb("#FFFFFF"));
    assert_eq!(solved.shift, Shift::Lighter);
    assert!(worst_contrast(solved.colour, &backdrop) >= 3.0);
    let gap = hue_gap(Oklch::from_rgb(pick).h, Oklch::from_rgb(solved.colour).h);
    assert!(gap <= 3.0, "hue moved {gap}");
}

#[test]
fn light_pick_on_light_floor_moves_darker() {
    let backdrop = flat(&["#F3F2ED"]);
    let solved = solve_accent(rgb("#FFE9A8"), &backdrop, rgb("#000000"));
    assert_eq!(solved.shift, Shift::Darker);
    assert!(worst_contrast(solved.colour, &backdrop) >= 3.0);
}

#[test]
fn no_pass_falls_back() {
    let backdrop = Backdrop::Range { min: 0.0, max: 1.0 };
    let solved = solve_accent(rgb("#CAFF33"), &backdrop, rgb("#123456"));
    assert_eq!(solved.colour, rgb("#123456"));
    assert_eq!(solved.shift, Shift::Fallback);
}

#[test]
fn sweep_always_passes_or_falls_back() {
    for backdrop in [gece(), flat(&["#F3F2ED"])] {
        for hue in (0..360).step_by(15) {
            for lightness in 1..=9 {
                for chroma in [0.05, 0.2] {
                    let pick = Oklch {
                        l: f64::from(lightness) / 10.0,
                        c: chroma,
                        h: f64::from(hue),
                    }
                    .to_rgb();
                    let solved = solve_accent(pick, &backdrop, rgb("#808080"));
                    assert!(
                        solved.shift == Shift::Fallback
                            || worst_contrast(solved.colour, &backdrop) >= 3.0,
                        "{}",
                        pick.to_hex()
                    );
                }
            }
        }
    }
}

fn assert_bar(plain: &PlainInk) {
    let backdrop = Backdrop::Flat {
        lums: vec![plain.floor.luminance(), plain.floor_high.luminance()],
    };
    let name = plain.floor.to_hex();
    assert!(worst_contrast(plain.ink, &backdrop) >= 7.0, "ink on {name}");
    assert!(
        worst_contrast(plain.muted, &backdrop) >= 4.5,
        "muted on {name}"
    );
    assert!(
        worst_contrast(plain.faint, &backdrop) >= 3.0,
        "faint on {name}"
    );
}

#[test]
fn derive_plain_bar_holds() {
    for floor in ["#0B0B0A", "#1C2420", "#F3F2ED", "#2A1A14", "#D1DDEC"] {
        let plain = derive_plain(rgb(floor)).unwrap_or_else(|| panic!("no plain for {floor}"));
        assert_bar(&plain);
    }
}

#[test]
fn derive_plain_dark_flag() {
    assert!(derive_plain(rgb("#1C2420")).unwrap().dark);
    assert!(!derive_plain(rgb("#F3F2ED")).unwrap().dark);
}

#[test]
fn neutral_floor_gives_neutral_ink() {
    for floor in ["#000000", "#FFFFFF", "#202020"] {
        let plain = derive_plain(rgb(floor)).unwrap_or_else(|| panic!("no plain for {floor}"));
        for role in [plain.ink, plain.muted, plain.faint] {
            let [r, g, b] = role.0.map(i16::from);
            assert!(
                (r - g).abs() <= 1 && (g - b).abs() <= 1 && (r - b).abs() <= 1,
                "{} on {floor} is tinted",
                role.to_hex()
            );
        }
        assert_bar(&plain);
    }
}

#[test]
fn tinted_migrated_floors_keep_tinted_ink() {
    for floor in ["#F3F2ED", "#1C2420"] {
        let plain = derive_plain(rgb(floor)).unwrap();
        let tinted = [plain.ink, plain.muted, plain.faint]
            .iter()
            .any(|role| role.0[0] != role.0[1] || role.0[1] != role.0[2]);
        assert!(tinted, "ink on {floor} lost its tint");
    }
}

#[test]
fn mid_grey_floor() {
    if let Some(plain) = derive_plain(rgb("#777777")) {
        assert_bar(&plain);
    }
}
