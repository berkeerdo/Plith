use super::{current_user_sid, process_user_sid};

#[test]
fn own_process_sid_matches_the_token_sid() {
    let own = current_user_sid().unwrap();
    assert!(own.starts_with("S-1-"));
    assert_eq!(process_user_sid(std::process::id()).unwrap(), own);
}
