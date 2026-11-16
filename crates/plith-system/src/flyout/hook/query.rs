//! Win32 queries and requests the flyout hook makes about one window, plus the journal path.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{CloseHandle, FARPROC, HWND};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, SW_HIDE, SW_SHOWNA,
    ShowWindowAsync,
};
use windows::core::{BOOL, PWSTR, s, w};

use crate::flyout::{self, WindowFacts, journal};

pub(super) type GetWindowBand = unsafe extern "system" fn(HWND, *mut u32) -> BOOL;

/// Longest class name read; the host classes are all shorter.
pub(super) const CLASS_BUF: usize = 128;

fn hwnd(handle: isize) -> HWND {
    HWND(handle as *mut _)
}

/// Resolves the undocumented `GetWindowBand` export; `None` when this Windows lacks it.
pub(super) fn resolve_band_fn() -> Option<GetWindowBand> {
    // SAFETY: user32 is always loaded; the name is a valid C string.
    let proc: FARPROC = unsafe {
        let module = GetModuleHandleW(w!("user32.dll")).ok()?;
        GetProcAddress(module, s!("GetWindowBand"))
    };
    // SAFETY: the export has the signature `BOOL GetWindowBand(HWND, PDWORD)`.
    proc.map(|f| unsafe { std::mem::transmute::<_, GetWindowBand>(f) })
}

/// Reads the class name into `buf` and returns how many UTF-16 units it holds (0 on failure).
pub(super) fn class_name_into(window: HWND, buf: &mut [u16; CLASS_BUF]) -> usize {
    // SAFETY: `buf` is a valid writable buffer.
    let len = unsafe { GetClassNameW(window, buf) };
    usize::try_from(len).unwrap_or(0)
}

/// The image name of `pid` without folder and `.exe`; empty when it cannot be read.
pub(super) fn process_name(pid: u32) -> String {
    // SAFETY: standard open, query and close; the handle is closed on every path.
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 520];
        let mut len = 520u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut len,
        )
        .is_ok();
        let _ = CloseHandle(process);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let file = path.rsplit(['\\', '/']).next().unwrap_or_default();
        file.strip_suffix(".exe")
            .or_else(|| file.strip_suffix(".EXE"))
            .unwrap_or(file)
            .to_owned()
    }
}

/// Whether the window is visible right now.
pub(super) fn is_visible(handle: isize) -> bool {
    // SAFETY: a plain visibility query; a dead handle reports not visible.
    unsafe { IsWindowVisible(hwnd(handle)) }.as_bool()
}

/// Posts a hide request; `true` when it was accepted.
pub(super) fn post_hide(handle: isize) -> bool {
    // SAFETY: `ShowWindowAsync(SW_HIDE)` only posts the request to the window's thread, so it
    // cannot block on a hung process.
    unsafe { ShowWindowAsync(hwnd(handle), SW_HIDE) }.as_bool()
}

/// Posts a show request. SHOWNA, never SHOW: the shell parks this window and it must not
/// gain or take activation.
pub(super) fn post_show(handle: isize) {
    // SAFETY: `ShowWindowAsync` on a possibly dead handle fails harmlessly. It posts to the
    // window's own thread, so a hung Explorer cannot block the caller (any thread, also one
    // that is about to call `process::exit`).
    let _ = unsafe { ShowWindowAsync(hwnd(handle), SW_SHOWNA) };
}

/// The pid that owns `window` (0 when it cannot be read).
pub(super) fn window_pid(window: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer.
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut pid)) };
    pid
}

/// The window's Z-band through the resolved export; `None` without it or on failure.
pub(super) fn window_band(band_fn: Option<GetWindowBand>, window: HWND) -> Option<u32> {
    band_fn.and_then(|get| {
        let mut band = 0u32;
        // SAFETY: resolved export with the documented-by-use signature.
        let ok = unsafe { get(window, &raw mut band) }.as_bool();
        (ok && band != u32::MAX).then_some(band)
    })
}

/// A journaled handle that is still ours to show: the pure [`flyout::is_orphan`] decision
/// over the same facts the hide path reads (class, process, band), plus whether it is still a
/// window and hidden. A recycled handle naming another app's window fails the facts.
pub(super) fn is_orphan(handle: isize, band_fn: Option<GetWindowBand>) -> bool {
    let window = hwnd(handle);
    // SAFETY: `IsWindow` accepts any value and only reports whether it names a window.
    if !unsafe { IsWindow(Some(window)) }.as_bool() {
        return false;
    }
    let mut buf = [0u16; CLASS_BUF];
    let len = class_name_into(window, &mut buf);
    let facts = WindowFacts {
        class: String::from_utf16_lossy(&buf[..len]),
        process: process_name(window_pid(window)),
        band: window_band(band_fn, window),
    };
    flyout::is_orphan(&facts, true, is_visible(handle))
}

/// This boot's stamp for the journal: the wall clock minus the uptime, in unix seconds.
pub(super) fn current_boot() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    // SAFETY: no arguments; reads the tick count since boot (sleep included).
    let uptime_ms = unsafe { GetTickCount64() };
    journal::boot_stamp(now, uptime_ms)
}

/// `%LOCALAPPDATA%\Plith\flyout-hidden.txt`; `None` when the known folder cannot be read.
pub(super) fn default_journal_path() -> Option<PathBuf> {
    // SAFETY: a valid known folder id, default flags and the current user's token.
    let raw =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: on success `raw` is a NUL terminated string the shell allocated; it is copied
    // and then freed exactly once with the COM allocator, as the API requires.
    let folder = unsafe {
        let folder = OsString::from_wide(raw.as_wide());
        CoTaskMemFree(Some(raw.0 as *const _));
        folder
    };
    Some(
        PathBuf::from(folder)
            .join("Plith")
            .join("flyout-hidden.txt"),
    )
}
