//! Installer logic with no Windows dependency: versions, the install situation, the file
//! whitelist, the worker report wire format and command-line parsing.

mod args;
mod files;
mod situation;
mod version;
mod wire;

#[cfg(test)]
mod tests;

pub use args::{Job, Mode, parse_args, quote_arg};
pub use files::is_ours;
pub use situation::{Situation, situation};
pub use version::Version;
pub use wire::{Report, decode, encode};
