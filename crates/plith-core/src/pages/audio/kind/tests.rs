use super::*;

#[test]
fn form_factor_decides_first() {
    assert_eq!(device_kind(Some(3), &["Speakers"]), DeviceKind::Headphones);
    assert_eq!(device_kind(Some(5), &[]), DeviceKind::Headphones);
    assert_eq!(device_kind(Some(9), &["Headset"]), DeviceKind::Display);
}

#[test]
fn other_form_factors_fall_back_to_the_names() {
    assert_eq!(
        device_kind(Some(1), &["Logitech G733 Headset"]),
        DeviceKind::Headphones
    );
    assert_eq!(
        device_kind(Some(1), &["Edifier R1280"]),
        DeviceKind::Speaker
    );
    assert_eq!(device_kind(Some(10), &[]), DeviceKind::Speaker);
}

#[test]
fn names_match_case_insensitively_and_turkish_safe() {
    for name in [
        "Kulaklık",
        "KULAKLIK",
        "kulaklik",
        "Bluetooth Kulakl\u{131}k",
        "Galaxy Buds2",
        "AirPods Pro",
        "USB Headphones",
    ] {
        assert_eq!(device_kind(None, &[name]), DeviceKind::Headphones, "{name}");
    }
}

#[test]
fn any_of_the_names_can_match() {
    assert_eq!(
        device_kind(None, &["Speakers (Realtek)", "Wireless Headset"]),
        DeviceKind::Headphones
    );
    assert_eq!(
        device_kind(None, &["DELL U2720 (HDMI)"]),
        DeviceKind::Display
    );
    assert_eq!(
        device_kind(None, &["Realtek DisplayPort Audio"]),
        DeviceKind::Display
    );
}

#[test]
fn unknown_defaults_to_speaker() {
    assert_eq!(device_kind(None, &[]), DeviceKind::Speaker);
    assert_eq!(device_kind(None, &["Realtek Audio"]), DeviceKind::Speaker);
    assert_eq!(DeviceKind::default(), DeviceKind::Speaker);
}

#[test]
fn captions_are_the_copy_constants() {
    assert_eq!(DeviceKind::Headphones.caption(), "KULAKLIK");
    assert_eq!(DeviceKind::Speaker.caption(), "HOPARL\u{d6}R");
    assert_eq!(DeviceKind::Display.caption(), "EKRAN");
}
