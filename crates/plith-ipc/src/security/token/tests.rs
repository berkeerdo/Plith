use super::{current_token_facts, process_token_facts};

#[test]
fn own_process_facts_match_the_token_facts() {
    let own = current_token_facts().unwrap();
    assert!(own.sid.starts_with("S-1-"));
    assert!(own.integrity_rid >= 0x1000);
    assert!(!own.app_container);
    assert_eq!(process_token_facts(std::process::id()).unwrap(), own);
}
