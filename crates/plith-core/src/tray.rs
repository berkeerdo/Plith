//! The tray icon's pure parts: the menu model with its labels, and the Run value that starts
//! Plith with Windows. No Windows calls here; the shell and the app do those.

pub mod menu;
pub mod notice;
pub mod run_value;

/// The tooltip of the tray icon. A product name, the same in every language.
pub const TOOLTIP: &str = "Plith";
