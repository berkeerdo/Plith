//! Windows helpers shared by the Plith binaries: the log file, the display language, the Run key
//! and a blocking HTTPS GET. Pure logic stays in `plith-core`.

pub mod autostart;
pub mod http;
pub mod locale;
pub mod log;
