use super::{TokenFacts, server_is_trusted};

const MEDIUM: u32 = 0x2000;
const HIGH: u32 = 0x3000;
const LOW: u32 = 0x1000;

fn facts(sid: &str, integrity_rid: u32, app_container: bool) -> TokenFacts {
    TokenFacts {
        sid: sid.to_string(),
        integrity_rid,
        app_container,
    }
}

#[test]
fn same_user_same_level_is_trusted() {
    assert!(server_is_trusted(
        &facts("S-1-5-21-1", MEDIUM, false),
        &facts("S-1-5-21-1", MEDIUM, false)
    ));
}

#[test]
fn same_user_higher_level_is_trusted() {
    assert!(server_is_trusted(
        &facts("S-1-5-21-1", HIGH, false),
        &facts("S-1-5-21-1", MEDIUM, false)
    ));
}

#[test]
fn lower_level_server_is_rejected() {
    assert!(!server_is_trusted(
        &facts("S-1-5-21-1", LOW, false),
        &facts("S-1-5-21-1", MEDIUM, false)
    ));
}

#[test]
fn app_container_server_is_rejected() {
    assert!(!server_is_trusted(
        &facts("S-1-5-21-1", MEDIUM, true),
        &facts("S-1-5-21-1", MEDIUM, false)
    ));
}

#[test]
fn other_user_is_rejected_even_when_higher() {
    assert!(!server_is_trusted(
        &facts("S-1-5-21-2", HIGH, false),
        &facts("S-1-5-21-1", MEDIUM, false)
    ));
}
