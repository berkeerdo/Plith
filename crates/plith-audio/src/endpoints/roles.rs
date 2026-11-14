//! The default render endpoint per role. The service snapshot only carries the Multimedia
//! default; a default-device switch writes Console and Multimedia, so verifying or restoring
//! one needs both.

#[cfg(windows)]
use super::com::{ComGuard, create_enumerator, default_endpoint};

/// The default render endpoint ids for the two roles a default switch writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefaultRoles {
    /// Id of the Console role default, if there is one.
    pub console: Option<String>,
    /// Id of the Multimedia role default, if there is one.
    pub multimedia: Option<String>,
}

impl DefaultRoles {
    /// True when both roles point at `id` (compared case-insensitively).
    #[must_use]
    pub fn all_are(&self, id: &str) -> bool {
        [&self.console, &self.multimedia].iter().all(|role| {
            role.as_deref()
                .is_some_and(|got| got.eq_ignore_ascii_case(id))
        })
    }

    /// True when both roles are set and name the same endpoint.
    #[must_use]
    pub fn agree(&self) -> bool {
        match (&self.console, &self.multimedia) {
            (Some(console), Some(multimedia)) => console.eq_ignore_ascii_case(multimedia),
            _ => false,
        }
    }
}

/// Reads the Console and Multimedia render defaults straight from Windows (not from a
/// service snapshot), on the calling thread. `None` when Core Audio cannot be used.
#[cfg(windows)]
#[must_use]
pub fn read_default_roles() -> Option<DefaultRoles> {
    use windows::Win32::Media::Audio::{eConsole, eMultimedia, eRender};

    let _com = ComGuard::init().ok()?;
    let en = create_enumerator().ok()?;
    let id_of = |role| default_endpoint(&en, eRender, role).map(|endpoint| endpoint.id);
    Some(DefaultRoles {
        console: id_of(eConsole),
        multimedia: id_of(eMultimedia),
    })
}
