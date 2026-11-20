//! Unicode text on the clipboard. The settings window is the clipboard owner, so writing needs the
//! window that last opened; the UI thread owns both.

use std::cell::Cell;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

thread_local! {
    static OWNER: Cell<isize> = const { Cell::new(0) };
}

/// Remembers the window that owns clipboard writes on this thread.
pub(super) fn set_owner(hwnd: isize) {
    OWNER.with(|owner| owner.set(hwnd));
}

/// Forgets `hwnd` as the owner when it is the current one.
pub(super) fn clear_owner(hwnd: isize) {
    OWNER.with(|owner| {
        if owner.get() == hwnd {
            owner.set(0);
        }
    });
}

fn open(hwnd: HWND) -> bool {
    // Another program may hold the clipboard for a moment.
    for _ in 0..3 {
        // SAFETY: `hwnd` is a window of this thread (or null, which the call allows for reading).
        if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

/// The Unicode text on the clipboard, if any.
pub(super) fn read_text(hwnd: HWND) -> Option<String> {
    if !open(hwnd) {
        return None;
    }
    let text = read_open();
    // SAFETY: the clipboard was opened by `open` above on this thread.
    let _ = unsafe { CloseClipboard() };
    text
}

fn read_open() -> Option<String> {
    // SAFETY: the clipboard is open; the handle stays owned by the system.
    let handle = unsafe { GetClipboardData(u32::from(CF_UNICODETEXT.0)) }.ok()?;
    let global = HGLOBAL(handle.0);
    // SAFETY: `global` is the clipboard's own movable block; it is unlocked below.
    let ptr = unsafe { GlobalLock(global) }.cast::<u16>();
    if ptr.is_null() {
        return None;
    }
    // SAFETY: CF_UNICODETEXT data is a NUL-terminated UTF-16 string inside the locked block, so
    // the scan stops at the terminator and the slice covers only initialised units.
    let text = unsafe {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    };
    // SAFETY: balances the lock above.
    let _ = unsafe { GlobalUnlock(global) };
    Some(text)
}

/// Puts `text` on the clipboard as Unicode text. Does nothing when no settings window has been
/// opened on this thread or the clipboard stays busy.
pub fn set_clipboard(text: &str) {
    let raw = OWNER.with(Cell::get);
    if raw == 0 {
        return;
    }
    let hwnd = crate::handles::hwnd_of(raw);
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if !open(hwnd) {
        return;
    }
    write_open(&units);
    // SAFETY: the clipboard was opened by `open` above on this thread.
    let _ = unsafe { CloseClipboard() };
}

fn write_open(units: &[u16]) {
    let bytes = size_of_val(units);
    // SAFETY: allocates a movable block; ownership passes to the clipboard on success.
    let Ok(global) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) }) else {
        return;
    };
    // SAFETY: `global` was just allocated with room for `units`; it is unlocked right after.
    unsafe {
        let ptr = GlobalLock(global).cast::<u16>();
        if ptr.is_null() {
            let _ = GlobalFree(Some(global));
            return;
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        let _ = GlobalUnlock(global);
        // SAFETY (continued): the clipboard is open and emptied, so it takes `global` on success.
        if EmptyClipboard().is_err()
            || SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(global.0))).is_err()
        {
            let _ = GlobalFree(Some(global));
        }
    }
}
