//! What an installer run finds on the machine.

use super::Version;

/// The state of the machine when setup starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situation {
    /// Nothing installed.
    Fresh,
    /// The frozen WPF app is installed.
    OverWpf {
        /// The WPF version found.
        from: Version,
    },
    /// An older Rust build is installed.
    Upgrade {
        /// The installed version.
        from: Version,
    },
    /// The same version (or a broken entry) is installed.
    Repair,
    /// A newer build than this setup is installed.
    NewerInstalled {
        /// The installed version.
        installed: Version,
    },
}

/// Classifies the machine from the Apps entry `DisplayVersion` text and the WPF exe check.
#[must_use]
pub fn situation(own: Version, apps_entry: Option<&str>, has_wpf_exe: bool) -> Situation {
    let parsed = apps_entry.and_then(Version::parse);
    let is_wpf = has_wpf_exe || parsed.is_some_and(|v| (v.major, v.minor) < (0, 4));
    if is_wpf {
        return Situation::OverWpf {
            from: parsed.unwrap_or(Version::new(0, 3, 0)),
        };
    }
    match (apps_entry, parsed) {
        (None, _) => Situation::Fresh,
        (Some(_), None) => Situation::Repair,
        (Some(_), Some(v)) if v < own => Situation::Upgrade { from: v },
        (Some(_), Some(v)) if v == own => Situation::Repair,
        (Some(_), Some(v)) => Situation::NewerInstalled { installed: v },
    }
}
