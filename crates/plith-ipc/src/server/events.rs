//! What the pipe server reports: bind errors, the callback's answer and server events.

use super::Rejection;
use crate::{DecodeError, Message};

/// Why the server pipe could not be created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindError {
    /// Another server of this user already owns the pipe name.
    AlreadyRunning,
    /// The pipe name is held by a server that is not this user's (a possible squatter).
    Squatted,
    /// Any other Windows error, as text.
    Os(String),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "the OSD pipe is already served"),
            Self::Squatted => write!(f, "the OSD pipe name is held by another user"),
            Self::Os(text) => write!(f, "{text}"),
        }
    }
}

impl std::error::Error for BindError {}

/// What the message callback wants the server to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep reading.
    Continue,
    /// Stop serving.
    Stop,
}

/// What [`OsdServer::serve`] reports to its callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerEvent {
    /// A client opened the pipe and was admitted; `pid` is the client process as Windows
    /// reports it for the pipe (its `Hello`, delivered next, names the same pid).
    Connected {
        /// The client's process id from `GetNamedPipeClientProcessId`.
        pid: u32,
    },
    /// A client was dropped before admission. No `Connected` or `Disconnected` follows.
    Rejected(Rejection),
    /// A message from the connected client, or why its bytes could not be decoded.
    Message(Result<Message, DecodeError>),
    /// The client closed the pipe or was dropped after a fatal decode error. Not reported
    /// when the server itself stops.
    Disconnected,
}
