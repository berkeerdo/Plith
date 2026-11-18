use super::*;
use crate::pipe_name;

fn level(percent: u8, label: &str) -> Message {
    Message::Level {
        meter: Meter::Volume,
        percent,
        muted: false,
        label: label.to_string(),
        muted_word: String::new(),
    }
}

fn level_with_word(label: &str, muted_word: &str) -> Message {
    Message::Level {
        meter: Meter::Volume,
        percent: 10,
        muted: true,
        label: label.to_string(),
        muted_word: muted_word.to_string(),
    }
}

#[test]
fn every_message_round_trips() {
    let labels = ["", "Speakers", "Hoparlör", &"a".repeat(64)];
    let mut all = vec![
        Message::Hello {
            client_pid: 0xDEAD_BEEF,
        },
        Message::Hide,
        Message::Bye,
        Message::Ping,
    ];
    for label in labels {
        for meter in [Meter::Volume, Meter::Brightness] {
            for muted in [false, true] {
                all.push(Message::Level {
                    meter,
                    percent: 100,
                    muted,
                    label: label.to_string(),
                    muted_word: "SESSİZ".to_string(),
                });
            }
        }
    }
    for message in all {
        let bytes = encode(&message).unwrap();
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn level_round_trips_the_muted_word() {
    for word in ["", "STUMM", "SESSİZ", "MUDO", &"é".repeat(32)] {
        let message = level_with_word("Speakers", word);
        let bytes = encode(&message).unwrap();
        assert_eq!(decode(&bytes), Ok(message), "word {word:?}");
    }
}

#[test]
fn level_wire_bytes_are_exact() {
    let bytes = encode(&level(42, "A")).unwrap();
    assert_eq!(bytes, [0x50, 0x4F, 2, 2, 0, 42, 0, 1, 0x41, 0]);
    let bytes = encode(&level_with_word("A", "MUDO")).unwrap();
    assert_eq!(
        bytes,
        [
            0x50, 0x4F, 2, 2, 0, 10, 1, 1, 0x41, 4, b'M', b'U', b'D', b'O'
        ]
    );
    assert_eq!(
        encode(&Message::Hello { client_pid: 1 }).unwrap(),
        [0x50, 0x4F, 2, 1, 1, 0, 0, 0]
    );
    assert_eq!(encode(&Message::Hide).unwrap(), [0x50, 0x4F, 2, 3]);
    assert_eq!(encode(&Message::Bye).unwrap(), [0x50, 0x4F, 2, 4]);
    assert_eq!(encode(&Message::Ping).unwrap(), [0x50, 0x4F, 2, 5]);
}

#[test]
fn ping_with_a_payload_is_rejected() {
    assert_eq!(decode(&[0x50, 0x4F, 2, 5, 0]), Err(DecodeError::Trailing));
}

#[test]
fn longest_legal_message_fits() {
    let longest = level_with_word(&"é".repeat(32), &"é".repeat(32));
    let bytes = encode(&longest).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE);
    assert_eq!(bytes.len(), 8 + MAX_LABEL + 1 + MAX_LABEL);
}

#[test]
fn label_over_64_bytes_does_not_encode() {
    assert_eq!(
        encode(&level(1, &"a".repeat(65))),
        Err(EncodeError::LabelTooLong)
    );
    assert_eq!(
        encode(&level(1, &"é".repeat(33))),
        Err(EncodeError::LabelTooLong)
    );
}

#[test]
fn an_overlong_muted_word_is_cut() {
    // Bounded like the label: the encoder refuses it, the decoder rejects it.
    assert_eq!(
        encode(&level_with_word("A", &"a".repeat(65))),
        Err(EncodeError::LabelTooLong)
    );
    let mut bytes = vec![0x50, 0x4F, 2, 2, 0, 5, 1, 0, 65];
    bytes.extend(std::iter::repeat_n(b'a', 65));
    assert_eq!(decode(&bytes), Err(DecodeError::Label));
}

#[test]
fn a_version_1_frame_is_rejected() {
    let version_1_level = [0x50, 0x4F, 1, 2, 0, 42, 0, 1, 0x41];
    let error = decode(&version_1_level).unwrap_err();
    assert_eq!(error, DecodeError::Version(1));
    assert!(error.is_fatal());
    assert_eq!(
        decode(&[0x50, 0x4F, 1, 1, 1, 0, 0, 0]),
        Err(DecodeError::Version(1))
    );
}

#[test]
fn percent_over_100_is_rejected_both_ways() {
    assert_eq!(encode(&level(101, "")), Err(EncodeError::PercentOutOfRange));
    let bytes = [0x50, 0x4F, 2, 2, 0, 101, 0, 0, 0];
    assert_eq!(decode(&bytes), Err(DecodeError::Percent(101)));
}

#[test]
fn truncated_input_is_short() {
    let bytes = encode(&level_with_word("Hoparlör", "SESSİZ")).unwrap();
    for len in 0..4 {
        assert_eq!(decode(&bytes[..len]), Err(DecodeError::Short));
    }
    for len in 4..bytes.len() {
        assert_eq!(
            decode(&bytes[..len]),
            Err(DecodeError::Short),
            "prefix {len}"
        );
    }
    assert_eq!(
        decode(&[0x50, 0x4F, 2, 1, 0, 0, 0]),
        Err(DecodeError::Short)
    );
}

#[test]
fn wrong_magic_and_other_versions_are_fatal() {
    let magic = decode(&[b'X', b'O', 2, 3]).unwrap_err();
    assert_eq!(magic, DecodeError::Magic);
    assert!(magic.is_fatal());
    let version = decode(&[b'P', b'O', 3, 3]).unwrap_err();
    assert_eq!(version, DecodeError::Version(3));
    assert!(version.is_fatal());
    assert_eq!(decode(&[b'P', b'O', 0, 3]), Err(DecodeError::Version(0)));
}

#[test]
fn unknown_kind_is_not_fatal() {
    let error = decode(&[b'P', b'O', 2, 9]).unwrap_err();
    assert_eq!(error, DecodeError::UnknownKind(9));
    assert!(!error.is_fatal());
}

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(decode(&[0x50, 0x4F, 2, 3, 0]), Err(DecodeError::Trailing));
    assert_eq!(decode(&[0x50, 0x4F, 2, 4, 0]), Err(DecodeError::Trailing));
    let mut hello = encode(&Message::Hello { client_pid: 7 }).unwrap();
    hello.push(0);
    assert_eq!(decode(&hello), Err(DecodeError::Trailing));
    let mut lvl = encode(&level(5, "x")).unwrap();
    lvl.push(0);
    assert_eq!(decode(&lvl), Err(DecodeError::Trailing));
}

#[test]
fn invalid_utf8_label_is_rejected() {
    let bytes = [0x50, 0x4F, 2, 2, 0, 5, 0, 1, 0xFF, 0];
    assert_eq!(decode(&bytes), Err(DecodeError::Label));
}

#[test]
fn invalid_utf8_muted_word_is_rejected() {
    let bytes = [0x50, 0x4F, 2, 2, 0, 5, 0, 0, 1, 0xFF];
    assert_eq!(decode(&bytes), Err(DecodeError::Label));
}

#[test]
fn muted_flag_2_is_rejected() {
    assert_eq!(
        decode(&[0x50, 0x4F, 2, 2, 0, 5, 2, 0, 0]),
        Err(DecodeError::Flag(2))
    );
}

#[test]
fn unknown_meter_is_rejected() {
    assert_eq!(
        decode(&[0x50, 0x4F, 2, 2, 7, 5, 0, 0, 0]),
        Err(DecodeError::Meter(7))
    );
}

#[test]
fn errors_display_and_are_std_errors() {
    fn is_error<E: std::error::Error>(_: &E) {}
    is_error(&EncodeError::LabelTooLong);
    is_error(&DecodeError::Short);
    assert!(!DecodeError::Version(3).to_string().is_empty());
    assert!(!EncodeError::PercentOutOfRange.to_string().is_empty());
}

#[test]
fn pipe_name_holds_session_and_sid() {
    assert_eq!(
        pipe_name(3, "S-1-5-21-1-2-3-1001"),
        r"\\.\pipe\plith-osd-3-S-1-5-21-1-2-3-1001"
    );
    assert_ne!(pipe_name(3, "S-1-5-21-9"), pipe_name(3, "S-1-5-21-8"));
}

#[test]
fn label_over_64_bytes_does_not_decode() {
    let mut bytes = vec![0x50, 0x4F, 2, 2, 0, 5, 0, 65];
    bytes.extend(std::iter::repeat_n(b'a', 65));
    bytes.push(0);
    assert_eq!(decode(&bytes), Err(DecodeError::Label));
}
