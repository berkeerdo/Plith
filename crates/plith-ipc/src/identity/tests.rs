use super::{ServerTrust, TokenFacts, server_is_trusted};

const MEDIUM: u32 = 0x2000;
const MEDIUM_PLUS: u32 = 0x2100;
const HIGH: u32 = 0x3000;
const LOW: u32 = 0x1000;

fn facts(sid: &str, integrity_rid: u32, app_container: bool) -> TokenFacts {
    TokenFacts {
        sid: sid.to_string(),
        integrity_rid,
        app_container,
    }
}

fn medium_client() -> TokenFacts {
    facts("S-1-5-21-1", MEDIUM, false)
}

#[test]
fn installed_trust_rejects_a_same_level_server() {
    assert!(!server_is_trusted(
        &facts("S-1-5-21-1", MEDIUM, false),
        &medium_client(),
        ServerTrust::Installed
    ));
}

#[test]
fn installed_trust_accepts_high_and_medium_plus() {
    for level in [HIGH, MEDIUM_PLUS] {
        assert!(server_is_trusted(
            &facts("S-1-5-21-1", level, false),
            &medium_client(),
            ServerTrust::Installed
        ));
    }
}

#[test]
fn dev_trust_accepts_a_same_level_server() {
    assert!(server_is_trusted(
        &facts("S-1-5-21-1", MEDIUM, false),
        &medium_client(),
        ServerTrust::DevSameUser
    ));
}

#[test]
fn lower_level_server_is_rejected_by_both() {
    for trust in [ServerTrust::Installed, ServerTrust::DevSameUser] {
        assert!(!server_is_trusted(
            &facts("S-1-5-21-1", LOW, false),
            &medium_client(),
            trust
        ));
    }
}

#[test]
fn app_container_server_is_rejected_by_both() {
    for trust in [ServerTrust::Installed, ServerTrust::DevSameUser] {
        assert!(!server_is_trusted(
            &facts("S-1-5-21-1", HIGH, true),
            &medium_client(),
            trust
        ));
    }
}

#[test]
fn other_user_is_rejected_even_when_higher() {
    for trust in [ServerTrust::Installed, ServerTrust::DevSameUser] {
        assert!(!server_is_trusted(
            &facts("S-1-5-21-2", HIGH, false),
            &medium_client(),
            trust
        ));
    }
}
