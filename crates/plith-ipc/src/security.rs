//! Access control for the OSD pipe.

#[cfg(windows)]
pub(crate) mod token;

#[cfg(test)]
mod tests;

/// Returns the SDDL of the OSD pipe for one user.
///
/// The DACL grants full access to that user only and blocks inheritance. The mandatory
/// label is medium with no-write-up, so the medium integrity notch can write to a UIAccess
/// server while lower integrity processes cannot.
#[must_use]
pub fn pipe_sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{user_sid})S:(ML;;NW;;;ME)")
}
