//! The Start Menu link: an `IShellLinkW` saved through `IPersistFile`.

use std::path::Path;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

/// Balances a `CoInitializeEx` that this scope made.
struct ComScope(bool);

impl ComScope {
    /// Initialises COM (apartment threaded) on this thread unless it already is, in either mode.
    fn enter() -> Result<Self, String> {
        // SAFETY: plain initialisation of the calling thread.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result == RPC_E_CHANGED_MODE {
            // Already initialised as multithreaded: usable, and not ours to undo.
            return Ok(Self(false));
        }
        result
            .ok()
            .map_err(|error| format!("cannot start COM: {error}"))?;
        Ok(Self(true))
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` of `enter`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Writes `link` (a `.lnk` path) pointing at `target`, starting in `working_dir`. Saved beside
/// the link as `.lnk.new` and renamed over it, so a failure never leaves a half-written link.
pub(super) fn write(link: &Path, target: &Path, working_dir: &Path) -> Result<(), String> {
    let _com = ComScope::enter()?;
    let staged = link.with_extension("lnk.new");
    if let Some(folder) = link.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("cannot create {}: {error}", folder.display()))?;
    }
    save(&staged, target, working_dir)
        .map_err(|error| format!("cannot write {}: {error}", link.display()))?;
    std::fs::rename(&staged, link).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        format!("cannot replace {}: {error}", link.display())
    })
}

fn save(path: &Path, target: &Path, working_dir: &Path) -> windows::core::Result<()> {
    // SAFETY: COM is initialised by the caller; the strings outlive each call.
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        shell_link.SetPath(&HSTRING::from(target.as_os_str()))?;
        shell_link.SetWorkingDirectory(&HSTRING::from(working_dir.as_os_str()))?;
        shell_link.SetDescription(&HSTRING::from("Plith"))?;
        shell_link.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)?;
        let file: IPersistFile = shell_link.cast()?;
        file.Save(&HSTRING::from(path.as_os_str()), true)
    }
}

/// The target of a saved link; for tests.
#[cfg(test)]
pub(super) fn read_target(link: &Path) -> Result<String, String> {
    use windows::Win32::Foundation::MAX_PATH;
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    let _com = ComScope::enter()?;
    // SAFETY: COM is initialised; the buffer is `MAX_PATH` wide as the call requires.
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let file: IPersistFile = shell_link.cast().map_err(|error| error.to_string())?;
        file.Load(
            &HSTRING::from(link.as_os_str()),
            windows::Win32::System::Com::STGM(0),
        )
        .map_err(|error| error.to_string())?;
        let mut buffer = [0u16; MAX_PATH as usize];
        let mut find = WIN32_FIND_DATAW::default();
        shell_link
            .GetPath(&mut buffer, &raw mut find, 0)
            .map_err(|error| error.to_string())?;
        let end = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        Ok(String::from_utf16_lossy(&buffer[..end]))
    }
}
