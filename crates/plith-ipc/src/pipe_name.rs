/// Returns the pipe path for the OSD helper of one user in one Windows session.
///
/// The result is `\\.\pipe\plith-osd-<session_id>-<user_sid>`. The pipe namespace is shared
/// by every session, so the user SID keeps another user from pre-creating this user's name.
#[must_use]
pub fn pipe_name(session_id: u32, user_sid: &str) -> String {
    format!(r"\\.\pipe\plith-osd-{session_id}-{user_sid}")
}
