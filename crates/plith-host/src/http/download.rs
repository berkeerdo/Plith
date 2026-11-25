//! A blocking HTTPS download to a file: written as `<name>.part`, renamed when complete.

use super::{WINHTTP_QUERY_CONTENT_LENGTH, number_header, open, read_chunk};
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A file larger than this is refused (the setup is about 10 MB; this is a safety stop).
const MAX_FILE: u64 = 200 * 1024 * 1024;

/// The temporary name a download is written under before the rename.
fn part_path(dest: &Path) -> PathBuf {
    let mut name = OsString::from(dest.file_name().unwrap_or_default());
    name.push(".part");
    dest.with_file_name(name)
}

/// Whole percent of `total` that `done` is, 0 to 100; 0 when the total is unknown.
fn percent(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    u8::try_from((done.min(total) * 100) / total).unwrap_or(100)
}

/// Downloads `url` to `dest`. HTTPS only; redirects are followed (GitHub serves release assets
/// from another host), and the caller decides by signature, not by host, whether the file is
/// trusted. The bytes go to `<dest>.part`, which is renamed to `dest` when complete and deleted
/// on any failure. `progress` gets whole percents, 0 to 100, as they change (never when the
/// server sends no length).
///
/// # Errors
///
/// A short message for a non-HTTPS URL, a failed call, a status other than 200, a file over
/// 200 MB, or a write or rename failure.
pub fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(u8)) -> Result<(), String> {
    let part = part_path(dest);
    let result = fetch_to(url, &part, progress);
    let result =
        result.and_then(|()| std::fs::rename(&part, dest).map_err(|e| format!("rename: {e}")));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn fetch_to(url: &str, part: &Path, progress: &mut dyn FnMut(u8)) -> Result<(), String> {
    let exchange = open(url, None)?;
    let total = number_header(&exchange.request, WINHTTP_QUERY_CONTENT_LENGTH).map_or(0, u64::from);
    if total > MAX_FILE {
        return Err("file too large".to_string());
    }
    let mut file = File::create(part).map_err(|e| format!("create: {e}"))?;
    let mut chunk = vec![0u8; 64 * 1024];
    let (mut done, mut shown) = (0u64, None);
    loop {
        let read = read_chunk(&exchange.request, &mut chunk)?;
        if read == 0 {
            break;
        }
        done += read as u64;
        if done > MAX_FILE {
            return Err("file too large".to_string());
        }
        file.write_all(&chunk[..read])
            .map_err(|e| format!("write: {e}"))?;
        let now = percent(done, total);
        if total > 0 && shown != Some(now) {
            shown = Some(now);
            progress(now);
        }
    }
    file.flush().map_err(|e| format!("write: {e}"))?;
    if total > 0 && done != total {
        return Err("download ended early".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{part_path, percent};
    use std::path::Path;

    #[test]
    fn part_file_sits_beside_the_target() {
        assert_eq!(
            part_path(Path::new(r"C:\u\Plith-Setup-0.5.0.exe")),
            Path::new(r"C:\u\Plith-Setup-0.5.0.exe.part")
        );
    }

    #[test]
    fn percent_is_whole_clamped_and_zero_for_unknown_length() {
        assert_eq!(percent(50, 200), 25);
        assert_eq!(percent(300, 200), 100);
        assert_eq!(percent(5, 0), 0);
    }
}
