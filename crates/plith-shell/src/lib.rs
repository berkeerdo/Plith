//! The notch window: hit testing, input, hover, topmost handling, fullscreen detection, OLE drag and drop and the tray.

mod handles;
pub mod hit;
pub mod monitor;
pub mod poster;
pub mod process;
mod suppress;
mod topmost;
pub mod window;

pub use monitor::{MonitorInfo, is_remote_session, primary_monitor};
pub use poster::{QuitHandle, ShapePoster, UiWaker};
pub use process::{init_process, init_ui_thread, uninit_ui_thread, user_busy};
pub use window::{Shell, ShellEvent, guard, run_message_loop};
