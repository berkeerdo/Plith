//! Windows helpers shared by the Plith binaries: the log file, the display language, the Run key,
//! a blocking HTTPS GET and the Plith signature check with its certificate stores. Pure logic
//! stays in `plith-core`.

pub mod autostart;
pub mod http;
pub mod locale;
pub mod log;
pub mod trust;
