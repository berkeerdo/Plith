//! Start with Windows: the HKCU Run value `Plith`.
//!
//! The value is the truth, not a setting of the file: the app reads it when the settings window
//! opens and writes or deletes it on a switch. Only `HKEY_CURRENT_USER` is touched. The text and
//! the "is it ours" rule are pure and live in `plith_core::tray::run_value`.

use plith_core::tray::run_value::{RUN_KEY, RUN_VALUE, is_ours, run_value};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

#[cfg(test)]
mod tests;

/// Something that holds the start with Windows switch. A trait so app tests never touch the
/// registry.
pub trait Autostart {
    /// Whether Windows starts Plith at sign in.
    fn is_on(&self) -> bool;

    /// Turns it on or off.
    ///
    /// # Errors
    /// The text of the Windows error when the value cannot be written or deleted.
    fn set(&mut self, on: bool) -> Result<(), String>;
}

/// A Run key under `HKEY_CURRENT_USER`, named by its subkey path.
#[derive(Debug, Clone)]
pub struct RunKey {
    subkey: HSTRING,
}

impl RunKey {
    /// The real Run key.
    #[must_use]
    pub fn user_run() -> Self {
        Self::at(RUN_KEY)
    }

    /// Any key under `HKEY_CURRENT_USER`, for tests.
    #[must_use]
    pub fn at(subkey: &str) -> Self {
        Self {
            subkey: HSTRING::from(subkey),
        }
    }

    /// The text of the value `Plith`, `None` when the key or the value is missing or the value
    /// is not a string.
    ///
    /// # Errors
    /// The text of an unexpected Windows error.
    pub fn read(&self) -> Result<Option<String>, String> {
        let Some(key) = Open::existing(&self.subkey, KEY_QUERY_VALUE)? else {
            return Ok(None);
        };
        let name = HSTRING::from(RUN_VALUE);
        let mut kind = REG_VALUE_TYPE(0);
        let mut bytes = 0u32;
        // SAFETY: `key` is open; with a null data pointer the call only reports type and size.
        let size = unsafe {
            RegQueryValueExW(
                key.0,
                &name,
                None,
                Some(&raw mut kind),
                None,
                Some(&raw mut bytes),
            )
        };
        if size == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(size)?;
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return Ok(None);
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        // SAFETY: the buffer holds `bytes` bytes, the size the first call reported.
        let got = unsafe {
            RegQueryValueExW(
                key.0,
                &name,
                None,
                None,
                Some(buffer.as_mut_ptr().cast::<u8>()),
                Some(&raw mut bytes),
            )
        };
        check(got)?;
        buffer.truncate((bytes as usize) / 2);
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        Ok(Some(String::from_utf16_lossy(&buffer)))
    }

    /// Writes the value `Plith`, creating the key when it is missing.
    ///
    /// # Errors
    /// The text of the Windows error.
    pub fn write(&self, text: &str) -> Result<(), String> {
        let key = Open::create(&self.subkey)?;
        let name = HSTRING::from(RUN_VALUE);
        let data: Vec<u8> = text
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        // SAFETY: `key` is open for writing and `data` is the NUL-terminated UTF-16 text.
        let code = unsafe { RegSetValueExW(key.0, &name, None, REG_SZ, Some(&data)) };
        check(code)
    }

    /// Deletes the value `Plith`; a missing key or value is already what was asked for.
    ///
    /// # Errors
    /// The text of an unexpected Windows error.
    pub fn delete(&self) -> Result<(), String> {
        let Some(key) = Open::existing(&self.subkey, KEY_SET_VALUE)? else {
            return Ok(());
        };
        let name = HSTRING::from(RUN_VALUE);
        // SAFETY: `key` is open for writing.
        let code = unsafe { RegDeleteValueW(key.0, &name) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(code)
    }
}

/// The switch on the real Run key for the exe at `exe`.
#[derive(Debug)]
pub struct RegistryAutostart {
    key: RunKey,
    exe: String,
}

impl RegistryAutostart {
    /// The switch for the running exe on the real Run key.
    ///
    /// # Errors
    /// The text of the error when the path of the running exe is unknown.
    pub fn for_running_exe() -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        Ok(Self::new(RunKey::user_run(), exe.display().to_string()))
    }

    /// A switch writing `exe` into `key`.
    #[must_use]
    pub fn new(key: RunKey, exe: String) -> Self {
        Self { key, exe }
    }
}

impl Autostart for RegistryAutostart {
    fn is_on(&self) -> bool {
        matches!(self.key.read(), Ok(Some(value)) if is_ours(&value))
    }

    fn set(&mut self, on: bool) -> Result<(), String> {
        if on {
            self.key.write(&run_value(&self.exe))
        } else {
            self.key.delete()
        }
    }
}

/// An open registry key, closed on drop.
struct Open(HKEY);

impl Open {
    /// Opens `subkey` under the current user; `None` when it does not exist.
    fn existing(
        subkey: &HSTRING,
        access: windows::Win32::System::Registry::REG_SAM_FLAGS,
    ) -> Result<Option<Open>, String> {
        let mut key = HKEY::default();
        // SAFETY: `subkey` is a NUL-terminated wide string and `key` receives the handle.
        let code = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                access,
                &raw mut key,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(code)?;
        Ok(Some(Open(key)))
    }

    /// Opens `subkey` under the current user for writing, creating it when missing.
    fn create(subkey: &HSTRING) -> Result<Open, String> {
        let mut key = HKEY::default();
        // SAFETY: `subkey` is a NUL-terminated wide string and `key` receives the handle.
        let code = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &raw mut key,
                None,
            )
        };
        check(code)?;
        Ok(Open(key))
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful open and is closed exactly once, here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn check(code: WIN32_ERROR) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("registry error {}", code.0))
    }
}
