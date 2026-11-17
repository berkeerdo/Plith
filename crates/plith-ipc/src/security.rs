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
