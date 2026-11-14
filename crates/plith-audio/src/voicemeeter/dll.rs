//! Run-time discovery and loading of `VoicemeeterRemote64.dll`.

use std::path::PathBuf;

use windows::Win32::Foundation::{ERROR_SUCCESS, HMODULE};
use windows::Win32::System::LibraryLoader::LoadLibraryW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_SAM_FLAGS, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use windows::core::PCWSTR;

use super::decode::install_dir_from_uninstall;

/// File name of the Remote API library.
pub(super) const DLL_NAME: &str = "VoicemeeterRemote64.dll";
const UNINSTALL_KEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\VB:Voicemeeter {17359A74-1236-5467}";
const DEFAULT_DIR: &str = r"C:\Program Files (x86)\VB\Voicemeeter";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Reads `UninstallString` from the Voicemeeter uninstall key in one registry view.
fn read_uninstall_string(view: REG_SAM_FLAGS) -> Option<String> {
    let key_name = wide(UNINSTALL_KEY);
    let value_name = wide("UninstallString");
    let mut key = HKEY::default();
    // SAFETY: `key_name` is NUL terminated and outlives the call; `key` is a valid out pointer.
    let rc = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key_name.as_ptr()),
            None,
            KEY_READ | view,
            &raw mut key,
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let mut buffer = vec![0u16; 1024];
    let mut size = u32::try_from(buffer.len() * 2).unwrap_or(0);
    let mut kind = REG_VALUE_TYPE::default();
    // SAFETY: `key` is open; the data pointer addresses `size` writable bytes, `value_name` is
    // NUL terminated, and all out pointers are valid for the call.
    let rc = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&raw mut kind),
            Some(buffer.as_mut_ptr().cast::<u8>()),
            Some(&raw mut size),
        )
    };
    // SAFETY: `key` was opened above and is closed exactly once.
    unsafe {
        let _ = RegCloseKey(key);
    }
    if rc != ERROR_SUCCESS || kind != REG_SZ {
        return None;
    }
    let units = (size as usize / 2).min(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..units]);
    Some(text.trim_end_matches('\0').to_owned())
}

/// Candidate DLL locations in priority order: registry (64 then 32 bit view), the default
/// install directory, then the bare name for the default search path.
fn candidates() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        if let Some(dir) = read_uninstall_string(view).and_then(|s| install_dir_from_uninstall(&s))
        {
            let path = PathBuf::from(dir).join(DLL_NAME);
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }
    let fallback = PathBuf::from(DEFAULT_DIR).join(DLL_NAME);
    if !found.contains(&fallback) {
        found.push(fallback);
    }
    found
}

fn load(path: &str) -> Option<HMODULE> {
    let name = wide(path);
    // SAFETY: `name` is a NUL terminated wide string that outlives the call.
    unsafe { LoadLibraryW(PCWSTR(name.as_ptr())) }.ok()
}

/// Loads the Remote API DLL. Tries each known install directory that really holds the file,
/// then the default search path. The error is a short human readable reason.
pub(super) fn load_remote_dll() -> Result<HMODULE, String> {
    for path in candidates() {
        if path.is_file()
            && let Some(module) = path.to_str().and_then(load)
        {
            return Ok(module);
        }
    }
    load(DLL_NAME).ok_or_else(|| format!("{DLL_NAME} not found (Voicemeeter is not installed)"))
}
