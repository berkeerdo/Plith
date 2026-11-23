use super::display_off;
use windows::Win32::System::SystemServices::{
    GUID_CONSOLE_DISPLAY_STATE, GUID_SESSION_DISPLAY_STATUS,
};

#[test]
fn the_session_display_state_drives_display_off() {
    assert_eq!(display_off(&GUID_SESSION_DISPLAY_STATUS, 4, 0), Some(true));
    assert_eq!(display_off(&GUID_SESSION_DISPLAY_STATUS, 4, 1), Some(false));
    // Dimmed is still on: the notch stays.
    assert_eq!(display_off(&GUID_SESSION_DISPLAY_STATUS, 4, 2), Some(false));
}

#[test]
fn the_console_display_state_is_ignored() {
    // Over RDP the console monitor sleeps while the remote session is on screen
    // (2026-11-23: the notch froze 15 minutes into an RDP session).
    assert_eq!(display_off(&GUID_CONSOLE_DISPLAY_STATE, 4, 0), None);
}

#[test]
fn a_short_payload_is_ignored() {
    assert_eq!(display_off(&GUID_SESSION_DISPLAY_STATUS, 3, 0), None);
}
