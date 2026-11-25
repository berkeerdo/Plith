//! Access control for the OSD pipe.

#[cfg(windows)]
pub(crate) mod token;

#[cfg(test)]
mod tests;

/// The integrity level RID of a medium (standard, not elevated) process.
pub const MEDIUM_RID: u32 = 0x2000;

/// Returns the SDDL of the OSD pipe for one user.
///
/// The DACL grants full access to that user only and blocks inheritance. The mandatory
/// label is medium with no-write-up, so the medium integrity notch can write to a UIAccess
/// server while lower integrity processes cannot.
#[must_use]
pub fn pipe_sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{user_sid})S:(ML;;NW;;;ME)")
}

/// Returns the SDDL of the installer's report pipe for one user.
///
/// The DACL grants full access to that user and to the built-in administrators (`BA`: the
/// elevated worker may run as another administrator account than the window) and blocks
/// inheritance. The label is medium with no-write-up, as in [`pipe_sddl`].
#[must_use]
pub fn setup_pipe_sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{user_sid})(A;;GA;;;BA)S:(ML;;NW;;;ME)")
}

/// Pure. Whether an integrity level RID is above medium (medium plus, high, system).
#[must_use]
pub fn is_above_medium(integrity_rid: u32) -> bool {
    integrity_rid > MEDIUM_RID
}

/// Whether this process runs above medium integrity (for example elevated). The installed
/// helper then runs at the same level, and [`ServerTrust::Installed`](crate::ServerTrust)
/// refuses it, so the link fails closed: the notch should log that instead of retrying.
///
/// # Errors
/// The text says why the process token could not be read.
#[cfg(windows)]
pub fn own_integrity_above_medium() -> Result<bool, String> {
    token::current_token_facts()
        .map(|facts| is_above_medium(facts.integrity_rid))
        .map_err(|e| format!("{e:?}"))
}

/// Whether process `pid` runs above medium integrity.
///
/// # Errors
/// The text says why the process or its token could not be read.
#[cfg(windows)]
pub fn process_integrity_above_medium(pid: u32) -> Result<bool, String> {
    token::process_token_facts(pid)
        .map(|facts| is_above_medium(facts.integrity_rid))
        .map_err(|e| format!("{e:?}"))
}

/// The string form of the current user's SID, such as `S-1-5-21-...`.
///
/// # Errors
/// The text says why the process token could not be read.
#[cfg(windows)]
pub fn current_user_sid() -> Result<String, String> {
    token::current_user_sid()
}
