use super::*;
use crate::theme::builtin::BUILTIN;
use crate::theme::fixtures::HALKA;

fn rgb(text: &str) -> Rgb {
    Rgb::parse(text).unwrap()
}

fn gece_text() -> String {
    BUILTIN[0].manifest.to_owned()
}

/// The Gece manifest with one line replaced (or appended when the key is absent).
fn gece_with(key: &str, value: &str) -> String {
    let prefix = format!("{key}=");
    let mut found = false;
    let mut lines: Vec<String> = BUILTIN[0]
        .manifest
        .lines()
        .map(|l| {
            if l.starts_with(&prefix) {
                found = true;
                format!("{key}={value}")
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !found {
        lines.push(format!("{key}={value}"));
    }
    lines.join("\n")
}

fn gece_without(key: &str) -> String {
    let prefix = format!("{key}=");
    BUILTIN[0]
        .manifest
        .lines()
        .filter(|l| !l.starts_with(&prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

fn reason(text: &str) -> (String, &'static str) {
    let err = parse_pack(text).unwrap_err();
    (err.key, err.reason)
}

#[test]
fn gece_builtin_parses() {
    let pack = parse_pack(BUILTIN[0].manifest).unwrap();
    assert_eq!(pack.id, "gece");
    assert_eq!(pack.name, "Gece");
    assert_eq!(
        pack.open.floor,
        Floor::Colour {
            top: rgb("#0B0B0A"),
            high: rgb("#151412")
        }
    );
    assert_eq!(pack.open.ink.ink, rgb("#ECE9E2"));
    assert_eq!(pack.open.ink.muted, rgb("#A39E94"));
    assert_eq!(pack.open.ink.faint, rgb("#8A847A"));
    assert_eq!(pack.open.ink.track, rgb("#4A463F"));
    assert_eq!(pack.open.ink.faint_track, rgb("#2E2B27"));
    assert_eq!(pack.open.ink.border, rgb("#2B2824"));
    assert_eq!(pack.flow, rgb("#CAFF33"));
    assert_eq!(pack.flow_style, FlowStyle::Fill);
    assert!(pack.flow_adjustable && pack.floor_adjustable && pack.dark);
    assert_eq!(pack.signature, Signature::Still);
    assert_eq!(pack.swatches.len(), 6);
    assert_eq!(pack.blurbs.len(), 8);
    assert_eq!(pack.collapsed, pack.open);
}

#[test]
fn crlf_bom_spaces_parse_same() {
    let clean = parse_pack(&gece_text()).unwrap();
    let body = gece_text().lines().collect::<Vec<_>>().join("  \r\n  ");
    let messy = format!("\u{feff}  {body}  \r\n");
    assert_eq!(parse_pack(&messy).unwrap(), clean);
}

#[test]
fn duplicate_key_refused() {
    let text = format!("{}\nname=Again", gece_text());
    assert_eq!(reason(&text), ("name".to_owned(), "duplicate key"));
}

#[test]
fn unknown_key_refused() {
    let text = format!("{}\nglow.size=3", gece_text());
    assert_eq!(reason(&text), ("glow.size".to_owned(), "unknown key"));
    let text = format!("{}\nformat=2", gece_without("format"));
    assert_eq!(reason(&text).1, "unknown format");
}

#[test]
fn missing_flow_refused() {
    assert_eq!(
        reason(&gece_without("flow.color")),
        ("flow.color".to_owned(), "missing required key")
    );
}

#[test]
fn bad_colour_refused() {
    assert_eq!(
        reason(&gece_with("flow.color", "#CAFF3")),
        ("flow.color".to_owned(), "bad colour")
    );
}

#[test]
fn bad_number_and_list_refused() {
    assert_eq!(reason(&HALKA.replace("0.78", "wide")).1, "bad number");
    assert_eq!(
        reason(&HALKA.replace("24,40,572,180", "24,40,572")).1,
        "bad list"
    );
    assert_eq!(
        reason(&gece_with("flow.swatches", "#CAFF33,,#FFB224")).1,
        "bad list"
    );
}

#[test]
fn tintable_refused() {
    let err = parse_pack(&gece_with("material.tintable", "true")).unwrap_err();
    assert_eq!(err.key, "material.tintable");
    assert_eq!(err.reason, "tinting is not built");
    assert!(parse_pack(&gece_with("material.tintable", "false")).is_ok());
}

#[test]
fn material_floor_not_adjustable() {
    let text = format!("{HALKA}floor.adjustable=true\n");
    assert_eq!(reason(&text).0, "floor.adjustable");
}

#[test]
fn low_contrast_refused() {
    let (key, why) = reason(&gece_with("ink.faint", "#3A3733"));
    assert_eq!(key, "ink.faint");
    assert!(why.contains("contrast"));
    let (key, _) = reason(&gece_with("ink.primary", "#6A665F"));
    assert_eq!(key, "ink.primary");
    let (key, _) = reason(&gece_with("flow.color", "#222222"));
    assert_eq!(key, "flow.color");
}

#[test]
fn failing_swatch_refused() {
    let (key, why) = reason(&gece_with("flow.swatches", "#CAFF33,#1A1A1A"));
    assert_eq!(key, "flow.swatches");
    assert!(why.contains("contrast"));
}

#[test]
fn too_many_swatches_refused() {
    let seven = "#CAFF33,#FFB224,#FFE9A8,#7FD4FF,#FF7A90,#B9A4FF,#FFFFFF";
    assert_eq!(
        reason(&gece_with("flow.swatches", seven)),
        ("flow.swatches".to_owned(), "more than 6 swatches")
    );
}

#[test]
fn punch_skips_flow_check() {
    let text = "format=1
id=serit
name=Şerit
blurb.en=Cream tape
floor.kind=color
floor.color=#F8E9CC
floor.color_high=#F3E1BC
ink.primary=#2A2113
ink.muted=#5A4B2E
ink.faint=#6E5C39
ink.track=#B9A57A
ink.faint_track=#D4C29C
ink.border=#CDB88B
flow.color=#2A2113
flow.style=punch
motion.open=feed
";
    let pack = parse_pack(text).unwrap();
    assert_eq!(pack.flow_style, FlowStyle::Punch);
    assert_eq!(pack.signature, Signature::Feed);
    assert!(!pack.dark);
    let low = text
        .replace("flow.style=punch", "flow.style=fill")
        .replace("flow.color=#2A2113", "flow.color=#F0E0B8");
    assert_eq!(reason(&low).0, "flow.color");
}

#[test]
fn material_manifest_parses() {
    let pack = parse_pack(HALKA).unwrap();
    assert_eq!(
        pack.open.floor,
        Floor::Material {
            lum_min: 0.78,
            lum_max: 0.93,
            mean: rgb("#CCFA0D")
        }
    );
    assert_eq!(pack.text_zone, Some([24.0, 40.0, 572.0, 180.0]));
    assert_eq!(pack.signature, Signature::Rings);
    assert_eq!(pack.overlays, ["ring-1", "ring-2", "ring-3"]);
    assert!(!pack.floor_adjustable && !pack.dark);
    assert_eq!(pack.collapsed.ink.ink, rgb("#ECE9E2"));
    assert!(matches!(pack.collapsed.floor, Floor::Colour { .. }));
    assert_eq!(
        pack.backdrop(&pack.open),
        Backdrop::Range {
            min: 0.78,
            max: 0.93
        }
    );
}

#[test]
fn material_range_inverted_refused() {
    let text = HALKA.replace("floor.lum_min=0.78", "floor.lum_min=0.95");
    assert_eq!(
        reason(&text),
        ("floor.lum_min".to_owned(), "lum_min above lum_max")
    );
}

#[test]
fn backdrop_is_never_empty() {
    let pack = parse_pack(&gece_text()).unwrap();
    match pack.backdrop(&pack.open) {
        Backdrop::Flat { lums } => assert_eq!(lums.len(), 2),
        Backdrop::Range { .. } => panic!("a colour floor is flat"),
    }
}

#[test]
fn blurb_falls_back_to_en() {
    let pack = parse_pack(HALKA).unwrap();
    assert_eq!(pack.blurb("tr"), "Lime fabric");
    let gece = parse_pack(&gece_text()).unwrap();
    assert_eq!(gece.blurb("tr"), "Sade siyah, malzemesiz");
    assert_eq!(gece.blurb("xx"), "Plain black, no material");
}

#[test]
fn collapsed_defaults_to_open() {
    let pack = parse_pack(&gece_with("collapsed.ink.border", "#444444")).unwrap();
    assert_eq!(pack.collapsed.floor, pack.open.floor);
    assert_eq!(pack.collapsed.ink.ink, pack.open.ink.ink);
    assert_eq!(pack.collapsed.ink.border, rgb("#444444"));
}

#[test]
fn id_and_required_keys_checked() {
    assert_eq!(reason(&gece_with("id", "Gece")).0, "id");
    assert_eq!(reason(&gece_without("name")).0, "name");
    assert_eq!(reason(&gece_without("blurb.en")).0, "blurb.en");
    assert_eq!(reason(&gece_without("floor.kind")).0, "floor.kind");
    assert_eq!(
        reason(&gece_without("floor.color_high")).0,
        "floor.color_high"
    );
    assert_eq!(reason(&gece_without("ink.border")).0, "ink.border");
    assert_eq!(reason(&gece_without("format")).0, "format");
}

#[test]
fn pack_error_displays_key_and_reason() {
    let err = PackError::new("flow.color", "bad colour");
    assert_eq!(err.to_string(), "`flow.color`: bad colour");
}

#[test]
fn halka_builtin_parses() {
    let pack = parse_pack(BUILTIN[1].manifest).unwrap();
    assert_eq!(pack.id, "halka");
    assert_eq!(pack.signature, Signature::Rings);
    assert_eq!(pack.overlays, ["ring-1", "ring-2", "ring-3", "disc"]);
    assert_eq!(pack.blurbs.len(), 8);
    assert_eq!(
        pack.open.floor,
        Floor::Material {
            lum_min: 0.5773,
            lum_max: 0.9275,
            mean: rgb("#CCFA0D")
        }
    );
    assert_eq!(pack.text_zone, Some([30.0, 12.0, 566.0, 394.0]));
    assert_eq!(pack.flow, rgb("#000000"));
    assert_eq!(pack.collapsed_flow, Some(rgb("#CAFF33")));
    assert_eq!(pack.swatches.len(), 5);
    assert!(pack.flow_adjustable && !pack.floor_adjustable && !pack.dark);
    let gece = parse_pack(BUILTIN[0].manifest).unwrap();
    assert_eq!(pack.collapsed, gece.open);
}

#[test]
fn material_needs_mean() {
    let text = HALKA.replace("floor.mean=#CCFA0D\n", "");
    assert_eq!(
        reason(&text),
        ("floor.mean".to_owned(), "missing required key")
    );
}

#[test]
fn collapsed_flow_checked_on_collapsed_face() {
    let ok = format!("{HALKA}collapsed.flow.color=#CAFF33\n");
    assert_eq!(
        parse_pack(&ok).unwrap().collapsed_flow,
        Some(rgb("#CAFF33"))
    );
    // Dark on the black pill: fine on the lime open face, refused on the pill.
    let bad = format!("{HALKA}collapsed.flow.color=#141A05\n");
    let (key, why) = reason(&bad);
    assert_eq!(key, "collapsed.flow.color");
    assert!(why.contains("contrast"));
}
