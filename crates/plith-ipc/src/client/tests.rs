use super::*;

#[test]
fn a_new_status_is_down_with_no_error() {
    let status = LinkStatus::default();
    assert!(!status.is_connected());
    assert_eq!(status.last_error(), None);
}

#[test]
fn a_failure_is_kept_until_the_next_connect() {
    let status = LinkStatus::default();
    status.failed(String::from("open: not found"));
    assert!(!status.is_connected());
    assert_eq!(status.last_error().as_deref(), Some("open: not found"));
    status.failed(String::from("open: busy"));
    assert_eq!(
        status.last_error().as_deref(),
        Some("open: busy"),
        "the latest failure wins"
    );
    status.connected();
    assert!(status.is_connected());
    assert_eq!(status.last_error(), None, "a connect clears the error");
    status.failed(String::from("write: broken pipe"));
    assert!(!status.is_connected(), "a failed write drops the link");
    assert_eq!(status.last_error().as_deref(), Some("write: broken pipe"));
}

#[test]
fn an_untrusted_server_has_its_own_text() {
    assert!(UNTRUSTED.contains("not trusted"), "{UNTRUSTED}");
}
