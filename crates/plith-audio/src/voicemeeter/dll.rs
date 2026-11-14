//! Run-time discovery and loading of `VoicemeeterRemote64.dll`.

use windows::Win32::Foundation::{ERROR_SUCCESS, HMODULE};
use windows::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_SAM_FLAGS, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use windows::core::PCWSTR;

use super::decode::{candidate_paths, install_dir_from_uninstall};

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

/// Candidate DLL paths in priority order: registry (64 then 32 bit view), then the default
/// install directory. Always absolute: the DLL is never searched for by bare name.
fn candidates() -> Vec<String> {
    let dirs: Vec<String> = [KEY_WOW64_64KEY, KEY_WOW64_32KEY]
        .into_iter()
        .filter_map(|view| read_uninstall_string(view).and_then(|s| install_dir_from_uninstall(&s)))
        .collect();
    candidate_paths(&dirs, DEFAULT_DIR, DLL_NAME)
}

/// Loads one absolute path, searching only its own directory and System32 for dependencies.
fn load(path: &str) -> Option<HMODULE> {
    let name = wide(path);
    // SAFETY: `name` is a NUL terminated wide string that outlives the call; the flags make
    // the loader use only the DLL's own directory and System32 (no current directory, no PATH).
    unsafe {
        LoadLibraryExW(
            PCWSTR(name.as_ptr()),
            None,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    }
    .ok()
}

/// Loads the Remote API DLL from a known absolute install location only. The error is a short
/// human readable reason.
pub(super) fn load_remote_dll() -> Result<HMODULE, String> {
    for path in candidates() {
        if std::path::Path::new(&path).is_file()
            && let Some(module) = load(&path)
        {
            return Ok(module);
        }
    }
    Err("Voicemeeter is not installed".to_owned())
}
