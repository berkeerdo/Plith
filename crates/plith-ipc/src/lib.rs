//! The named pipe protocol between the notch process and the UIAccess OSD helper.
//!
//! This crate is pure: it only encodes and decodes messages. The pipe server and
//! client live elsewhere and move the bytes.

mod message;
mod pipe_name;

pub use message::{
    DecodeError, EncodeError, MAX_LABEL, MAX_MESSAGE, Message, Meter, PROTOCOL_VERSION, decode,
    encode,
};
pub use pipe_name::pipe_name;
