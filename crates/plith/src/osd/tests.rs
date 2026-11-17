use super::level_message;
use plith_core::osd::{OsdLevel, OsdMeter};
use plith_ipc::{Message, Meter, encode};

#[test]
fn a_volume_level_maps_field_by_field() {
    let message = level_message(OsdLevel {
        meter: OsdMeter::Volume,
        percent: 42,
        muted: true,
        label: String::from("Speakers"),
    });
    assert_eq!(
        message,
        Message::Level {
            meter: Meter::Volume,
            percent: 42,
            muted: true,
            label: String::from("Speakers"),
        }
    );
}

#[test]
fn a_brightness_level_maps_to_the_brightness_meter() {
    let message = level_message(OsdLevel {
        meter: OsdMeter::Brightness,
        percent: 100,
        muted: false,
        label: String::from("PARLAKLIK"),
    });
    assert!(matches!(
        message,
        Message::Level {
            meter: Meter::Brightness,
            percent: 100,
            muted: false,
            ..
        }
    ));
}

#[test]
fn a_full_length_label_still_encodes() {
    let label = "a".repeat(plith_core::osd::MAX_LABEL_BYTES);
    let message = level_message(OsdLevel {
        meter: OsdMeter::Volume,
        percent: 0,
        muted: false,
        label,
    });
    assert!(encode(&message).is_ok());
}
