use super::pipe_sddl;

#[test]
fn sddl_grants_only_the_user_and_labels_medium() {
    assert_eq!(
        pipe_sddl("S-1-5-21-1-2-3-1001"),
        "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)S:(ML;;NW;;;ME)"
    );
}
