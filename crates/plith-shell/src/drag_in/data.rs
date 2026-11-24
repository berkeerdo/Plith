//! Reading what a drag carries: whether it has a file list, which formats it offers, and the
//! paths. The test uses `QueryGetData` (no data moves); the list is read only on a drop.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use windows::Win32::System::Com::{
    CoTaskMemFree, DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::GetClipboardFormatNameW;
use windows::Win32::System::Ole::{CF_HDROP, ReleaseStgMedium};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

/// How many formats the log line names at most.
const FORMATS_SHOWN: usize = 12;
/// The first registered (named) clipboard format id.
const FIRST_REGISTERED: u16 = 0xC000;
/// How many names a drop reads at most: the shelf keeps eight, so a big drop is cut here before
/// any per-path work (the rest only costs the count).
const READ_MAX: u32 = 64;

/// The request for a file list in a global-memory medium.
pub(crate) fn hdrop_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        // Allowed: the medium constant is a small positive bit flag.
        #[allow(clippy::cast_sign_loss)]
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

/// True when the data object offers a file list.
pub(crate) fn has_files(data: &IDataObject) -> bool {
    let format = hdrop_format();
    // SAFETY: `format` is a fully initialised FORMATETC that lives for the whole call.
    unsafe { data.QueryGetData(&raw const format) }.is_ok()
}

/// The name of one clipboard format, for the log.
fn format_name(id: u16) -> String {
    match id {
        1 => "text".to_owned(),
        2 => "bitmap".to_owned(),
        13 => "unicodetext".to_owned(),
        15 => "hdrop".to_owned(),
        _ if id >= FIRST_REGISTERED => {
            let mut buf = [0_u16; 64];
            // SAFETY: `buf` is a writable slice; the call fills at most its length.
            let n = unsafe { GetClipboardFormatNameW(u32::from(id), &mut buf) };
            usize::try_from(n)
                .ok()
                .filter(|&n| n > 0)
                .map_or_else(|| format!("#{id}"), |n| String::from_utf16_lossy(&buf[..n]))
        }
        _ => format!("#{id}"),
    }
}

/// The formats the data object offers, comma separated (at most a dozen).
pub(crate) fn formats(data: &IDataObject) -> String {
    // 1 is DATADIR_GET.
    // SAFETY: plain call on a live data object; the enumerator it returns owns itself.
    let Ok(list) = (unsafe { data.EnumFormatEtc(1) }) else {
        return "none".to_owned();
    };
    let mut names: Vec<String> = Vec::new();
    let mut one = [FORMATETC::default()];
    while names.len() < FORMATS_SHOWN {
        // SAFETY: `one` is a writable one-element slice; no fetched count is asked for, which
        // is allowed for a count of one.
        let status = unsafe { list.Next(&mut one, None) };
        if status.0 != 0 {
            break;
        }
        let name = format_name(one[0].cfFormat);
        if !one[0].ptd.is_null() {
            // SAFETY: a target device block, when present, is task-allocated for the caller.
            unsafe { CoTaskMemFree(Some(one[0].ptd.cast_const().cast())) };
            one[0].ptd = std::ptr::null_mut();
        }
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(",")
    }
}

/// The paths of the file list, or none when it cannot be read.
pub(crate) fn files(data: &IDataObject) -> Vec<PathBuf> {
    let format = hdrop_format();
    // SAFETY: `format` is fully initialised and lives for the call.
    let Ok(mut medium) = (unsafe { data.GetData(&raw const format) }) else {
        return Vec::new();
    };
    let paths = read_paths(&medium);
    // SAFETY: `medium` came from GetData and is released exactly once, after the last read.
    unsafe { ReleaseStgMedium(&raw mut medium) };
    paths
}

fn read_paths(medium: &STGMEDIUM) -> Vec<PathBuf> {
    // SAFETY: the medium was asked for in TYMED_HGLOBAL, so its union holds a global handle,
    // which is what an HDROP is.
    if medium.tymed != TYMED_HGLOBAL.0 as u32 {
        return Vec::new();
    }
    let hdrop = HDROP(unsafe { medium.u.hGlobal }.0);
    // SAFETY: `hdrop` is a live drop handle owned by `medium` for the whole function; a null
    // buffer asks for the count and the needed length.
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    (0..count.min(READ_MAX))
        .filter_map(|i| {
            // SAFETY: see above; `i` is below the count.
            let len = unsafe { DragQueryFileW(hdrop, i, None) } as usize;
            let mut buf = vec![0_u16; len + 1];
            // SAFETY: `buf` holds the name and its terminator.
            let n = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) } as usize;
            (n > 0).then(|| PathBuf::from(OsString::from_wide(&buf[..n.min(len)])))
        })
        .collect()
}
