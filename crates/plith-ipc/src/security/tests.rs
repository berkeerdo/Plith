use super::{MEDIUM_RID, is_above_medium, pipe_sddl, setup_pipe_sddl};

#[test]
fn setup_sddl_grants_the_user_and_administrators() {
    assert_eq!(
        setup_pipe_sddl("S-1-5-21-1-2-3-1001"),
        "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)(A;;GA;;;BA)S:(ML;;NW;;;ME)"
    );
}

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

#[cfg(windows)]
#[test]
fn a_process_reads_like_its_own_integrity() {
    let pid = std::process::id();
    assert_eq!(
        super::process_integrity_above_medium(pid),
        super::own_integrity_above_medium()
    );
}
