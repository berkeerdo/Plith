//! The data object of a drag out: the genuine shell data object for a set of paths, so the
//! target sees `CF_HDROP`, the shell ID list array and the drag image support Explorer's own
//! drags have. Nothing is copied; the object only names the files.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::System::Com::{CoTaskMemFree, IDataObject};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, IShellItemArray, SHCreateShellItemArrayFromIDLists, SHParseDisplayName,
};
use windows::core::{PCWSTR, Result};

/// A parsed shell path (an absolute item ID list), freed when dropped.
struct Pidl(*mut ITEMIDLIST);

impl Pidl {
    fn parse(path: &Path) -> Result<Self> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        // SAFETY: `wide` is a NUL-terminated path that outlives the call, and `pidl` is a
        // writable out pointer. On success the shell allocates the list; `Drop` frees it.
        unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &raw mut pidl, 0, None) }?;
        Ok(Self(pidl))
    }
}

impl Drop for Pidl {
    fn drop(&mut self) {
        // SAFETY: the list came from SHParseDisplayName, which allocates with the task
        // allocator, and is freed once.
        unsafe { CoTaskMemFree(Some(self.0.cast_const().cast())) };
    }
}

/// The shell data object for `paths`, or the error of the first step that fails. Paths that
/// cannot be parsed are left out; no path left is an error.
///
/// # Errors
/// Returns the shell error when no item can be bound or the object cannot be made.
pub(super) fn shell_data_object(paths: &[&Path]) -> Result<IDataObject> {
    let lists: Vec<Pidl> = paths.iter().filter_map(|p| Pidl::parse(p).ok()).collect();
    let raw: Vec<*const ITEMIDLIST> = lists.iter().map(|p| p.0.cast_const()).collect();
    // SAFETY: every pointer in `raw` is a live item ID list owned by `lists`, which outlives
    // the call; the array object copies what it needs.
    let array: IShellItemArray = unsafe { SHCreateShellItemArrayFromIDLists(&raw) }?;
    // SAFETY: plain bind on a live array; no bind context is needed.
    unsafe { array.BindToHandler(None, &BHID_DataObject) }
}
