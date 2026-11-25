//! Cancelling restart deletes that an earlier uninstall left for the install folder. Without this
//! a reinstall before the restart would lose its new setup copy (and the folder) at boot.
//!
//! Windows keeps them in `PendingFileRenameOperations`: pairs of source and destination, where
//! an empty destination means delete. Only delete pairs under the install root are dropped; every
//! other pair stays, in order.

use super::StepError;
use std::path::Path;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_MULTI_SZ, REG_VALUE_TYPE,
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{PCWSTR, w};

/// The prefix Windows puts on the paths of pending operations.
const NT_PREFIX: &str = r"\??\";

/// One list entry as the raw UTF-16 Windows stores: never converted, so an entry that is not ours
/// (an unpaired surrogate is legal in an NTFS name) is written back exactly as it was read.
pub(super) type Entry = Vec<u16>;

/// What the list held.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Pending {
    /// There is no list.
    Absent,
    /// The entries, pairs of source and destination.
    List(Vec<Entry>),
    /// The value is not a well-formed `REG_MULTI_SZ`; it must not be rewritten.
    Malformed,
}

/// Where the restart deletes are kept. A fake stands in for it in tests, so they never touch
/// `HKLM`.
pub(super) trait PendingDeletes {
    /// The list as it is now.
    ///
    /// # Errors
    /// The text of the error when the list exists but cannot be read.
    fn read(&mut self) -> Result<Pending, String>;

    /// Replaces the list, or removes it when `entries` is `None`.
    ///
    /// # Errors
    /// The text of the error when the list cannot be written.
    fn write(&mut self, entries: Option<&[Entry]>) -> Result<(), String>;
}

/// Drops the delete pairs (empty destination) whose source is `root` or lies under it, comparing
/// path components without case. Everything else is kept in order and untouched. `None` when the
/// entry count is odd: the list is not made of pairs, so it is not ours to edit.
pub(super) fn drop_pending_under(entries: &[Entry], root: &Path) -> Option<Vec<Entry>> {
    if !entries.len().is_multiple_of(2) {
        return None;
    }
    let root = components(&root.to_string_lossy());
    let mut kept = Vec::with_capacity(entries.len());
    for pair in entries.as_chunks::<2>().0 {
        // Strict decoding: an entry that is not valid UTF-16 cannot be under the root.
        let ours = pair[1].is_empty()
            && String::from_utf16(&pair[0])
                .is_ok_and(|source| components(&source).starts_with(&root));
        if !ours {
            kept.extend_from_slice(pair);
        }
    }
    Some(kept)
}

fn components(text: &str) -> Vec<String> {
    let text = text.strip_prefix(NT_PREFIX).unwrap_or(text);
    let text = text.strip_prefix(r"\\?\").unwrap_or(text);
    text.split('\\')
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Removes the pending deletes under `root`. A list that is absent, malformed, odd or without a
/// match is left alone (the malformed and odd ones with a log line).
pub(super) fn cancel_under(
    root: &Path,
    pending: &mut dyn PendingDeletes,
    log: &mut dyn FnMut(String),
) -> Result<(), StepError> {
    let fail = |reason: String| StepError::new(format!("pending restart deletes: {reason}"));
    let entries = match pending.read().map_err(fail)? {
        Pending::Absent => return Ok(()),
        Pending::Malformed => {
            log("the pending restart delete list is malformed; left untouched".to_string());
            return Ok(());
        }
        Pending::List(entries) => entries,
    };
    let Some(kept) = drop_pending_under(&entries, root) else {
        log("the pending restart delete list has an odd entry count; left untouched".to_string());
        return Ok(());
    };
    if kept.len() == entries.len() {
        return Ok(());
    }
    log("cancelling restart deletes left by an uninstall".to_string());
    let list = if kept.is_empty() { None } else { Some(kept) };
    pending.write(list.as_deref()).map_err(fail)
}

/// The real list in `HKLM`.
pub(super) struct SystemPending;

const KEY: PCWSTR = w!(r"SYSTEM\CurrentControlSet\Control\Session Manager");
const VALUE: PCWSTR = w!("PendingFileRenameOperations");

/// The open Session Manager key, closed on drop.
struct Open(HKEY);

impl Open {
    fn new() -> Result<Self, String> {
        let mut key = HKEY::default();
        // SAFETY: constant NUL-terminated names; `key` receives the handle.
        let code = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                KEY,
                None,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &raw mut key,
            )
        };
        if code == ERROR_SUCCESS {
            Ok(Self(key))
        } else {
            Err(format!("cannot open Session Manager: error {}", code.0))
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful open and is closed once, here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

impl PendingDeletes for SystemPending {
    fn read(&mut self) -> Result<Pending, String> {
        let key = Open::new()?;
        let mut kind = REG_VALUE_TYPE(0);
        let mut size = 0u32;
        // SAFETY: the first call only asks for the size; all pointers outlive it.
        let sized = unsafe {
            RegQueryValueExW(
                key.0,
                VALUE,
                None,
                Some(&raw mut kind),
                None,
                Some(&raw mut size),
            )
        };
        if sized == ERROR_FILE_NOT_FOUND {
            return Ok(Pending::Absent);
        }
        if sized != ERROR_SUCCESS || kind != REG_MULTI_SZ {
            return Err(format!("cannot read the list: error {}", sized.0));
        }
        let mut data = vec![0u8; size as usize];
        // SAFETY: `data` holds `size` bytes and outlives the call.
        let read = unsafe {
            RegQueryValueExW(
                key.0,
                VALUE,
                None,
                Some(&raw mut kind),
                Some(data.as_mut_ptr()),
                Some(&raw mut size),
            )
        };
        if read != ERROR_SUCCESS {
            return Err(format!("cannot read the list: error {}", read.0));
        }
        data.truncate(size as usize);
        Ok(decode(&data).map_or(Pending::Malformed, Pending::List))
    }

    fn write(&mut self, entries: Option<&[Entry]>) -> Result<(), String> {
        let key = Open::new()?;
        // SAFETY: constant NUL-terminated names and a byte slice alive for the call.
        let code = unsafe {
            match entries {
                None => RegDeleteValueW(key.0, VALUE),
                Some(entries) => {
                    let data = encode(entries);
                    RegSetValueExW(key.0, VALUE, None, REG_MULTI_SZ, Some(&data))
                }
            }
        };
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("cannot write the list: error {}", code.0))
        }
    }
}

/// `REG_MULTI_SZ` bytes into raw entries, by their terminators: every string ends in a NUL and
/// one more NUL ends the list, so an empty string before that (an empty destination) is an entry.
/// `None` when the bytes are not whole UTF-16 units or the terminators are missing.
pub(super) fn decode(data: &[u8]) -> Option<Vec<Entry>> {
    let (units, rest) = data.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let mut wide: Vec<u16> = units.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    if wide.len() <= 2 && wide.iter().all(|&unit| unit == 0) {
        return Some(Vec::new());
    }
    // The list terminator, then every entry must end in its own NUL.
    if wide.pop() != Some(0) || wide.last() != Some(&0) {
        return None;
    }
    wide.pop();
    Some(wide.split(|&unit| unit == 0).map(<[u16]>::to_vec).collect())
}

/// Entries into `REG_MULTI_SZ` bytes.
pub(super) fn encode(entries: &[Entry]) -> Vec<u8> {
    let mut wide: Vec<u16> = Vec::new();
    for entry in entries {
        wide.extend_from_slice(entry);
        wide.push(0);
    }
    wide.push(0);
    wide.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}
