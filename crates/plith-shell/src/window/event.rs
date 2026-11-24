//! What the notch window reports to the app.

use plith_core::display::Display;
use plith_core::scene::Suppression;
use std::path::PathBuf;

/// Something the app has to react to.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellEvent {
    /// The left button went down on the island and was released inside the drawn island shape,
    /// at `(x, y)` DIP relative to the island's top-left (the release point).
    Click {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
    },
    /// The left button went down on the island shape, at `(x, y)` island DIP.
    Press {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
    },
    /// The pressed pointer moved to a new point past the drag slop, at `(x, y)` island DIP. The
    /// point is not clamped to the shape and may lie outside the window.
    Drag {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
    },
    /// A drag ended (button up or lost capture) at `(x, y)` island DIP, not clamped to the
    /// shape. A press that never passed the slop ends as a `Click` instead.
    Release {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
    },
    /// The pointer moved over the window for the first time since it last left.
    Enter,
    /// The pointer moved to a new point inside the drawn island shape, at `(x, y)` DIP relative
    /// to the island's top-left. The shell does not know whether the island is open; the app
    /// decides what a move means.
    Move {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
    },
    /// The pointer left the window region (the island shape, ADR 0004).
    Leave,
    /// A drag with a file list entered the island (ADR 0018), at `(x, y)` island DIP (not
    /// clamped to the shape). `formats` names the clipboard formats the data object offers.
    DragEnter {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
        /// The offered formats, comma separated, for the log.
        formats: String,
        /// The effect granted: `copy`, `link` or `none`.
        effect: &'static str,
    },
    /// The dragged files moved to a new point over the island, at `(x, y)` island DIP.
    DragOver {
        /// Horizontal island DIP.
        x: f64,
        /// Vertical island DIP.
        y: f64,
    },
    /// The dragged files left the island without a drop.
    DragLeave,
    /// The files were dropped on the island. The shell has reset its mouse state.
    Drop {
        /// The dropped paths, in the order the source listed them.
        paths: Vec<PathBuf>,
    },
    /// A drag without a file list (text, a link, a source the integrity rules block) entered
    /// the island. It got no drop effect; nothing should open.
    DragRefused {
        /// The offered formats, comma separated, for the log.
        formats: String,
    },
    /// The DPI or the display configuration changed and the window was moved to this display;
    /// the render thread must follow it.
    DisplayChanged(Display),
    /// The window is about to be destroyed (`WM_CLOSE`): stop anything that presents to it.
    Closing,
    /// The suppression state changed; republish the scene with it.
    Suppression(Suppression),
    /// A service thread called [`UiWaker::wake`]; re-read whatever the services published.
    Wake,
    /// App timer `id` (set through [`TimerHandle`]) ticked.
    Timer(usize),
    /// A foreground change reached the topmost fight.
    Raised {
        /// Class name of the new foreground window, or `?` when it is already gone.
        foreground: String,
        /// Class name of the nearest visible window that sat above ours before the raise.
        /// `None` when nothing visible was above, or when the change was debounced.
        above: Option<String>,
        /// `true` when the windows were raised now, `false` when only the retry timer re-armed.
        immediate: bool,
    },
}
