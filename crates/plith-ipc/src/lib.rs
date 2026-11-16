//! The named pipe protocol between the notch process and the UIAccess OSD helper.
//!
//! The protocol ([`Message`], [`encode`], [`decode`]), the latest-wins [`latest::Slot`] and
//! the pipe ACL text ([`security::pipe_sddl`]) are pure and tested everywhere. The pipe
//! server ([`OsdServer`]), the non-blocking client ([`OsdLink`]) and [`launch_helper`]
//! (with the [`HelperProcess`] it returns) are Windows only.
//!
//! The README holds the API table, the threading notes and how the notch wires it in.

pub mod latest;
mod message;
mod pipe_name;
pub mod security;

#[cfg(windows)]
mod client;
#[cfg(windows)]
mod identity;
#[cfg(windows)]
mod launch;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use client::OsdLink;
#[cfg(windows)]
pub use identity::ServerTrust;
#[cfg(windows)]
pub use launch::{HelperProcess, INSTALLED_HELPER, current_pipe_name, launch_helper};
pub use message::{
    DecodeError, EncodeError, MAX_LABEL, MAX_MESSAGE, Message, Meter, PROTOCOL_VERSION, decode,
    encode,
};
pub use pipe_name::pipe_name;
#[cfg(windows)]
pub use server::{BindError, Flow, HELLO_TIMEOUT, OsdServer, Rejection, ServerEvent, Stopper};
