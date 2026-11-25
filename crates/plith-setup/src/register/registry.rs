//! The few registry calls the Apps entry needs: replace a key tree, write values, read one back.

use super::Hive;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

/// An open key, closed on drop.
pub(super) struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful open and is closed once, here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn hive_key(hive: Hive) -> HKEY {
    match hive {
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        Hive::CurrentUser => HKEY_CURRENT_USER,
    }
}

fn check(code: WIN32_ERROR, what: &str) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("{what}: registry error {}", code.0))
    }
}

/// Deletes `path` with everything under it; a key that is not there is fine.
pub(super) fn delete_tree(hive: Hive, path: &str) -> Result<(), String> {
    let wide = HSTRING::from(path);
    // SAFETY: a NUL-terminated wide subkey name, alive for the call.
    let code = unsafe { RegDeleteTreeW(hive_key(hive), PCWSTR(wide.as_ptr())) };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(code, &format!("delete {path}"))
}

/// Creates (or opens) `path` for writing values.
pub(super) fn create(hive: Hive, path: &str) -> Result<Key, String> {
    let wide = HSTRING::from(path);
    let mut key = HKEY::default();
    // SAFETY: a NUL-terminated wide subkey name; `key` receives the handle.
    let code = unsafe {
        RegCreateKeyExW(
            hive_key(hive),
            PCWSTR(wide.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &raw mut key,
            None,
        )
    };
    check(code, &format!("create {path}"))?;
    Ok(Key(key))
}

impl Key {
    /// Writes a `REG_SZ` value.
    pub(super) fn set_string(&self, name: &str, value: &str) -> Result<(), String> {
        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        self.set(name, REG_SZ, bytes_of_u16(&wide))
    }

    /// Writes a `REG_DWORD` value.
    pub(super) fn set_dword(&self, name: &str, value: u32) -> Result<(), String> {
        self.set(name, REG_DWORD, &value.to_le_bytes())
    }

    fn set(&self, name: &str, kind: REG_VALUE_TYPE, data: &[u8]) -> Result<(), String> {
        let wide = HSTRING::from(name);
        // SAFETY: a NUL-terminated wide value name and a byte slice, both alive for the call.
        let code = unsafe { RegSetValueExW(self.0, PCWSTR(wide.as_ptr()), None, kind, Some(data)) };
        check(code, &format!("write {name}"))
    }
}

fn bytes_of_u16(wide: &[u16]) -> &[u8] {
    // SAFETY: a `u16` slice is valid as twice as many bytes; the borrow keeps it alive.
    unsafe { std::slice::from_raw_parts(wide.as_ptr().cast(), wide.len() * 2) }
}

/// Reads a raw value; `None` when the key or the value is missing or unreadable.
fn read_raw(hive: Hive, path: &str, name: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
    let wide_path = HSTRING::from(path);
    let mut key = HKEY::default();
    // SAFETY: a NUL-terminated wide subkey name; `key` receives the handle.
    let opened = unsafe {
        RegOpenKeyExW(
            hive_key(hive),
            PCWSTR(wide_path.as_ptr()),
            None,
            KEY_QUERY_VALUE,
            &raw mut key,
        )
    };
    if opened != ERROR_SUCCESS {
        return None;
    }
    let key = Key(key);
    let wide_name = HSTRING::from(name);
    let mut kind = REG_VALUE_TYPE(0);
    let mut size = 0u32;
    // SAFETY: the first call only asks for the size; all pointers outlive it.
    let sized = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&raw mut kind),
            None,
            Some(&raw mut size),
        )
    };
    if sized != ERROR_SUCCESS {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` holds `size` bytes and outlives the call.
    let read = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&raw mut kind),
            Some(data.as_mut_ptr()),
            Some(&raw mut size),
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    data.truncate(size as usize);
    Some((kind, data))
}

/// A `REG_SZ` value as text; `None` when it is missing or another type.
pub(super) fn read_string(hive: Hive, path: &str, name: &str) -> Option<String> {
    let (kind, data) = read_raw(hive, path, name)?;
    if kind != REG_SZ {
        return None;
    }
    let wide: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let end = wide
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(wide.len());
    Some(String::from_utf16_lossy(&wide[..end]))
}

/// A `REG_DWORD` value; `None` when it is missing or another type.
#[cfg(test)]
pub(super) fn read_dword(hive: Hive, path: &str, name: &str) -> Option<u32> {
    let (kind, data) = read_raw(hive, path, name)?;
    if kind != REG_DWORD {
        return None;
    }
    Some(u32::from_le_bytes(data.try_into().ok()?))
}
