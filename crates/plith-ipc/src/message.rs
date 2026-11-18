//! Wire format of the notch to OSD protocol.

use std::fmt;

#[cfg(test)]
mod tests;

/// Protocol version carried in every message.
pub const PROTOCOL_VERSION: u8 = 2;
/// Largest encoded message in bytes: the header, a full label and a full muted word.
pub const MAX_MESSAGE: usize = 144;
/// Largest label, and largest muted word, in bytes (UTF-8).
pub const MAX_LABEL: usize = 64;

const KIND_HELLO: u8 = 1;
const KIND_LEVEL: u8 = 2;
const KIND_HIDE: u8 = 3;
const KIND_BYE: u8 = 4;
const KIND_PING: u8 = 5;

/// What a level message measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meter {
    /// Audio volume.
    Volume = 0,
    /// Display brightness.
    Brightness = 1,
}

/// One protocol message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// First message from a client.
    Hello {
        /// Process id of the client.
        client_pid: u32,
    },
    /// Show a level.
    Level {
        /// The measured quantity.
        meter: Meter,
        /// Level, 0 to 100.
        percent: u8,
        /// Whether the output is muted.
        muted: bool,
        /// Short text, at most [`MAX_LABEL`] bytes.
        label: String,
        /// The word for "muted" in the user's language, at most [`MAX_LABEL`] bytes.
        muted_word: String,
    },
    /// Hide the overlay.
    Hide,
    /// The client is leaving.
    Bye,
    /// Keepalive: the client is still there. Sent after [`crate::KEEPALIVE`] without another
    /// write; the server swallows it (it never reaches the `serve` callback).
    Ping,
}

/// Why a message could not be encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The label or the muted word is longer than [`MAX_LABEL`] bytes.
    LabelTooLong,
    /// The percent is above 100.
    PercentOutOfRange,
}

/// Why bytes could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer bytes than the message needs.
    Short,
    /// The first two bytes are not `PO`.
    Magic,
    /// A protocol version this build does not speak.
    Version(u8),
    /// A message kind this build does not know.
    UnknownKind(u8),
    /// Percent above 100.
    Percent(u8),
    /// Unknown meter id.
    Meter(u8),
    /// A flag byte that is neither 0 nor 1.
    Flag(u8),
    /// The label or the muted word is longer than [`MAX_LABEL`] bytes or not valid UTF-8.
    Label,
    /// Bytes remain after the message.
    Trailing,
}

impl DecodeError {
    /// Whether the connection cannot continue after this error.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Magic | Self::Version(_))
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LabelTooLong => write!(f, "label or muted word is longer than {MAX_LABEL} bytes"),
            Self::PercentOutOfRange => write!(f, "percent is above 100"),
        }
    }
}

impl std::error::Error for EncodeError {}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short => write!(f, "message is too short"),
            Self::Magic => write!(f, "bad magic bytes"),
            Self::Version(v) => write!(f, "unsupported protocol version {v}"),
            Self::UnknownKind(k) => write!(f, "unknown message kind {k}"),
            Self::Percent(p) => write!(f, "percent {p} is above 100"),
            Self::Meter(m) => write!(f, "unknown meter {m}"),
            Self::Flag(b) => write!(f, "invalid flag byte {b}"),
            Self::Label => write!(f, "label or muted word is too long or not valid UTF-8"),
            Self::Trailing => write!(f, "unexpected bytes after the message"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Encodes a message.
///
/// # Errors
/// Returns [`EncodeError`] when the percent or label is out of range.
pub fn encode(message: &Message) -> Result<Vec<u8>, EncodeError> {
    let mut out = vec![b'P', b'O', PROTOCOL_VERSION];
    match message {
        Message::Hello { client_pid } => {
            out.push(KIND_HELLO);
            out.extend_from_slice(&client_pid.to_le_bytes());
        }
        Message::Level {
            meter,
            percent,
            muted,
            label,
            muted_word,
        } => {
            if *percent > 100 {
                return Err(EncodeError::PercentOutOfRange);
            }
            let label_len = bounded_len(label)?;
            let word_len = bounded_len(muted_word)?;
            out.extend_from_slice(&[
                KIND_LEVEL,
                *meter as u8,
                *percent,
                u8::from(*muted),
                label_len,
            ]);
            out.extend_from_slice(label.as_bytes());
            out.push(word_len);
            out.extend_from_slice(muted_word.as_bytes());
        }
        Message::Hide => out.push(KIND_HIDE),
        Message::Bye => out.push(KIND_BYE),
        Message::Ping => out.push(KIND_PING),
    }
    Ok(out)
}

/// The length byte of `text`, which must fit [`MAX_LABEL`].
fn bounded_len(text: &str) -> Result<u8, EncodeError> {
    u8::try_from(text.len())
        .ok()
        .filter(|len| usize::from(*len) <= MAX_LABEL)
        .ok_or(EncodeError::LabelTooLong)
}

/// Decodes one message from exactly one frame of bytes.
///
/// Checks run in this order: length, magic, version, kind, payload.
///
/// # Errors
/// Returns [`DecodeError`] when the bytes are not a valid message.
pub fn decode(bytes: &[u8]) -> Result<Message, DecodeError> {
    let [m0, m1, version, kind, payload @ ..] = bytes else {
        return Err(DecodeError::Short);
    };
    if [*m0, *m1] != *b"PO" {
        return Err(DecodeError::Magic);
    }
    if *version != PROTOCOL_VERSION {
        return Err(DecodeError::Version(*version));
    }
    match *kind {
        KIND_HELLO => {
            let pid: [u8; 4] = payload
                .get(..4)
                .ok_or(DecodeError::Short)?
                .try_into()
                .map_err(|_| DecodeError::Short)?;
            no_trailing(&payload[4..])?;
            Ok(Message::Hello {
                client_pid: u32::from_le_bytes(pid),
            })
        }
        KIND_LEVEL => decode_level(payload),
        KIND_HIDE => no_trailing(payload).map(|()| Message::Hide),
        KIND_BYE => no_trailing(payload).map(|()| Message::Bye),
        KIND_PING => no_trailing(payload).map(|()| Message::Ping),
        other => Err(DecodeError::UnknownKind(other)),
    }
}

fn no_trailing(rest: &[u8]) -> Result<(), DecodeError> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(DecodeError::Trailing)
    }
}

fn decode_level(payload: &[u8]) -> Result<Message, DecodeError> {
    let [meter, percent, muted, label_len, rest @ ..] = payload else {
        return Err(DecodeError::Short);
    };
    let meter = match *meter {
        0 => Meter::Volume,
        1 => Meter::Brightness,
        other => return Err(DecodeError::Meter(other)),
    };
    if *percent > 100 {
        return Err(DecodeError::Percent(*percent));
    }
    let muted = match *muted {
        0 => false,
        1 => true,
        other => return Err(DecodeError::Flag(other)),
    };
    let (label, rest) = take_text(rest, usize::from(*label_len))?;
    let [word_len, rest @ ..] = rest else {
        return Err(DecodeError::Short);
    };
    let (muted_word, rest) = take_text(rest, usize::from(*word_len))?;
    no_trailing(rest)?;
    Ok(Message::Level {
        meter,
        percent: *percent,
        muted,
        label,
        muted_word,
    })
}

/// Splits `len` bytes of bounded UTF-8 text off the front of `bytes`.
fn take_text(bytes: &[u8], len: usize) -> Result<(String, &[u8]), DecodeError> {
    if len > MAX_LABEL {
        return Err(DecodeError::Label);
    }
    let (text, rest) = bytes.split_at_checked(len).ok_or(DecodeError::Short)?;
    let text = std::str::from_utf8(text).map_err(|_| DecodeError::Label)?;
    Ok((text.to_string(), rest))
}
