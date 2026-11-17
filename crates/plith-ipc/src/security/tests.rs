use super::{MEDIUM_RID, is_above_medium, pipe_sddl};

#[test]
fn sddl_grants_only_the_user_and_labels_medium() {
    assert_eq!(
        pipe_sddl("S-1-5-21-1-2-3-1001"),
        "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)S:(ML;;NW;;;ME)"
    );
}

#[test]
fn only_levels_above_medium_count_as_above_medium() {
    assert!(!is_above_medium(0x1000));
    assert!(!is_above_medium(MEDIUM_RID));
    assert!(is_above_medium(0x2100));
    assert!(is_above_medium(0x3000));
    assert!(is_above_medium(0x4000));
}

#[cfg(windows)]
#[test]
fn own_integrity_can_be_read() {
    assert!(super::own_integrity_above_medium().is_ok());
}
