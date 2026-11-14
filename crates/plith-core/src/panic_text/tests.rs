use super::*;

#[test]
fn str_and_string_payloads_keep_their_text() {
    assert_eq!(payload_text(&"plain"), "plain");
    assert_eq!(payload_text(&String::from("formatted 7")), "formatted 7");
}

#[test]
fn any_other_payload_is_named() {
    assert_eq!(payload_text(&42_u32), "non-string payload");
}
