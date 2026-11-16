//! Test helpers shared across modules.

use std::path::{Path, PathBuf};

/// A folder `plith-<name>-test-<pid>` under the temp directory holding at most a journal and
/// its temp file. Dropping it removes both files and the folder, so a test run leaves nothing.
pub(crate) struct ScratchDir {
    dir: PathBuf,
    journal: PathBuf,
}

impl ScratchDir {
    /// Creates the folder (empty journal path: any old file is removed).
    pub(crate) fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("plith-{name}-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp folder");
        let journal = dir.join("flyout-hidden.txt");
        let _ = std::fs::remove_file(&journal);
        Self { dir, journal }
    }

    /// The journal path inside the folder.
    pub(crate) fn journal(&self) -> &Path {
        &self.journal
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.journal);
        let _ = std::fs::remove_file(self.journal.with_extension("tmp"));
        let _ = std::fs::remove_dir(&self.dir);
    }
}
