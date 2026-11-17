use super::*;

#[test]
fn reads_nested_values_and_whitespace() {
    let v = parse(" { \"a\" : [1, -2.5e1, true, null, \"x\"], \"b\": {} } ").unwrap();
    let a = v.get("a").unwrap().as_arr().unwrap();
    assert_eq!(a[0], Json::Num(1.0));
    assert_eq!(a[1], Json::Num(-25.0));
    assert_eq!(a[2], Json::Bool(true));
    assert_eq!(a[3], Json::Null);
    assert_eq!(a[4].as_str(), Some("x"));
    assert_eq!(v.get("b"), Some(&Json::Obj(Vec::new())));
    assert_eq!(v.get("missing"), None);
}

#[test]
fn string_escapes_resolve() {
    let v = parse(r#""a\"b\\c\/d\n\t\u0130""#).unwrap();
    assert_eq!(v.as_str(), Some("a\"b\\c/d\n\t\u{130}"));
}

#[test]
fn surrogate_pair_makes_one_char() {
    let v = parse(r#""\ud83d\ude00""#).unwrap();
    assert_eq!(v.as_str(), Some("\u{1F600}"));
}

#[test]
fn lone_surrogates_are_errors() {
    assert!(parse(r#""\ud83d""#).is_err());
    assert!(parse(r#""\ud83dx""#).is_err());
    assert!(parse(r#""\ude00""#).is_err());
}

#[test]
fn raw_utf8_passes_through() {
    assert_eq!(parse("\"İstanbul\"").unwrap().as_str(), Some("İstanbul"));
}

#[test]
fn trailing_garbage_is_rejected() {
    assert!(parse("{} x").is_err());
    assert!(parse("1 2").is_err());
    assert!(parse("").is_err());
}

#[test]
fn malformed_bodies_are_errors() {
    for bad in [
        "{", "[1,", "{\"a\"}", "\"abc", "[1 2]", "nul", "-", "\"\\q\"", "{1:2}",
    ] {
        assert!(parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn depth_cap_stops_hostile_nesting() {
    let deep = "[".repeat(10_000) + &"]".repeat(10_000);
    assert!(parse(&deep).is_err());
    let ok = "[".repeat(60) + &"]".repeat(60);
    assert!(parse(&ok).is_ok());
}
