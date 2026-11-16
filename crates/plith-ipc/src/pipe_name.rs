/// Returns the pipe path for the OSD helper of one Windows session.
///
/// The result is `\\.\pipe\plith-osd-<session_id>`.
#[must_use]
pub fn pipe_name(session_id: u32) -> String {
    format!(r"\\.\pipe\plith-osd-{session_id}")
}
