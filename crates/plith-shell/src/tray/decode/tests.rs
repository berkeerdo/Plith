use super::*;

#[test]
fn a_left_click_opens_and_only_once() {
    assert_eq!(click_of(NIN_SELECT), Click::Open);
    assert_eq!(click_of(NIN_KEYSELECT), Click::Open);
    // The raw button messages arrive with the selection and must not open a second time.
    assert_eq!(click_of(0x201), Click::Ignore, "WM_LBUTTONDOWN");
    assert_eq!(click_of(0x202), Click::Ignore, "WM_LBUTTONUP");
    assert_eq!(click_of(0x203), Click::Ignore, "WM_LBUTTONDBLCLK");
}

#[test]
fn the_context_message_shows_the_menu_and_the_raw_right_button_does_not() {
    assert_eq!(click_of(WM_CONTEXTMENU), Click::Menu);
    assert_eq!(click_of(0x205), Click::Ignore, "WM_RBUTTONUP");
    assert_eq!(click_of(0x200), Click::Ignore, "WM_MOUSEMOVE");
    assert_eq!(click_of(0), Click::Ignore);
}

#[test]
fn the_event_is_the_low_word_of_lparam() {
    // The high word carries the icon id in version 4.
    assert_eq!(event_of(0x0001_0400), 0x400);
    assert_eq!(event_of(0x7B), 0x7B);
    assert_eq!(event_of(0x1234_5678), 0x5678);
}

#[test]
fn the_anchor_is_two_signed_words() {
    assert_eq!(anchor_of(0x0300_0100), (0x100, 0x300));
    // A monitor left of the primary has negative x.
    assert_eq!(anchor_of(0x0300_FF38), (-200, 0x300));
    assert_eq!(anchor_of(0xFF38_FF38), (-200, -200));
}

#[test]
fn the_tooltip_is_terminated_and_cut_to_fit() {
    let tip = fill_tip("Plith");
    assert_eq!(&tip[..5], &[80, 108, 105, 116, 104]);
    assert_eq!(tip[5], 0);
    let long = "x".repeat(300);
    let tip = fill_tip(&long);
    assert_eq!(tip[TIP_LEN - 1], 0);
    assert_eq!(tip[TIP_LEN - 2], u16::from(b'x'));
}
