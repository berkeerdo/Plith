//! Which first message admits a client. Pure: the kernel pid and the decoded message arrive
//! as arguments.

use std::time::Duration;

use crate::{DecodeError, Message};

/// How long a connected client has to send its `Hello` before it is dropped.
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(1);

/// Why a client was dropped before it was admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// No first message within [`HELLO_TIMEOUT`].
    Timeout,
    /// The client closed the pipe before its first message (for example a bind probe).
    ClosedBeforeHello,
    /// The first message was not a well-formed `Hello`.
    NotHello,
    /// `Hello` named another process than the one Windows reports for the pipe.
    PidMismatch {
        /// The pid from `GetNamedPipeClientProcessId`.
        kernel: u32,
        /// The pid the `Hello` claimed.
        hello: u32,
    },
    /// The client's pid could not be read from the pipe (Windows error text).
    NoClientPid(String),
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "no Hello within {} ms", HELLO_TIMEOUT.as_millis()),
            Self::ClosedBeforeHello => write!(f, "closed before Hello"),
            Self::NotHello => write!(f, "the first message was not Hello"),
            Self::PidMismatch { kernel, hello } => {
                write!(
                    f,
                    "Hello named pid {hello}, the pipe client is pid {kernel}"
                )
            }
            Self::NoClientPid(text) => write!(f, "client pid unknown: {text}"),
        }
    }
}

/// Admits a client whose first message is `Hello` naming `kernel_pid`, the pid Windows
/// reports for the pipe client. Returns that `Hello`, or why the client is refused.
///
/// # Errors
/// [`Rejection::NotHello`] for any other message or a decode error,
/// [`Rejection::PidMismatch`] when the pids differ.
pub fn admit(first: &Result<Message, DecodeError>, kernel_pid: u32) -> Result<Message, Rejection> {
    match first {
        Ok(Message::Hello { client_pid }) if *client_pid == kernel_pid => Ok(Message::Hello {
            client_pid: *client_pid,
        }),
        Ok(Message::Hello { client_pid }) => Err(Rejection::PidMismatch {
            kernel: kernel_pid,
            hello: *client_pid,
        }),
        _ => Err(Rejection::NotHello),
    }
}

#[cfg(test)]
mod tests;
