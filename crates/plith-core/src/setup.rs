//! Installer logic with no Windows dependency: versions, the install situation, the file
//! whitelist, the worker report wire format and command-line parsing.

mod args;
mod files;
mod flow;
mod screen;
mod situation;
mod version;
mod wire;

#[cfg(test)]
mod tests;

pub use args::{Job, Mode, parse_args, parse_pipe_name, quote_arg, setup_pipe_name};
pub use files::is_ours;
pub use flow::{Effect, Flow};
pub use screen::{
    BG_ART, BUTTON_ART, SETUP_H, SETUP_W, Screen, SetupAction, SetupTarget, setup_build, setup_hit,
};
pub use situation::{Situation, situation};
pub use version::Version;
pub use wire::{Report, decode, encode};
