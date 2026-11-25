//! A blocking HTTPS GET over `WinHTTP`. This file and its `download` child are the only code
//! that touches `WinHTTP`.

mod download;

use std::ffi::c_void;
use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_CONTENT_LENGTH,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect,
    WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::core::{PCWSTR, w};

pub use download::download;

/// Every phase (resolve, connect, send, receive) may take this long.
const TIMEOUT_MS: i32 = 10_000;
/// A body larger than this is refused.
const MAX_BODY: usize = 2 * 1024 * 1024;

/// The error text of a 404 reply, which a caller may treat as "nothing there yet".
pub const NOT_FOUND: &str = "http status 404";

/// A `WinHTTP` handle, closed on drop.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void, what: &str) -> Result<Self, String> {
        if raw.is_null() {
            Err(format!("{what}: {}", windows::core::Error::from_thread()))
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful WinHTTP open and is closed once.
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// Splits `https://host[:port]/path` into its parts. Anything but HTTPS is refused.
fn split(url: &str) -> Result<(&str, u16, &str), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or("only https is allowed")?;
    let (authority, path) = rest.find('/').map_or((rest, "/"), |i| rest.split_at(i));
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().map_err(|_| "bad port")?),
        None => (authority, 443),
    };
    if host.is_empty() {
        return Err("empty host".to_string());
    }
    Ok((host, port, path))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The `Accept` request header, without a terminating NUL: `WinHttpSendRequest` takes its length
/// from the slice, and a NUL inside that length makes the call fail with invalid parameter.
fn accept_header(accept: &str) -> Vec<u16> {
    format!("Accept: {accept}\r\n").encode_utf16().collect()
}

/// The handles of one request; the fields drop in order: request, connection, session.
struct Exchange {
    request: Handle,
    _connection: Handle,
    _session: Handle,
}

/// Sends a GET for `url` (with an `Accept` header when given), receives the response and checks
/// for status 200. `WinHTTP` follows redirects, but never one from HTTPS to plain HTTP.
fn open(url: &str, accept: Option<&str>) -> Result<Exchange, String> {
    let (host, port, path) = split(url)?;
    let headers = accept.map(accept_header);
    // SAFETY: plain WinHTTP calls; every handle is owned by a `Handle` and closed on drop, and
    // the wide strings outlive the calls that read them.
    unsafe {
        let session = Handle::new(
            WinHttpOpen(
                w!("Plith"),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ),
            "open",
        )?;
        WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS)
            .map_err(|e| format!("timeouts: {e}"))?;
        let host = wide(host);
        let connection = Handle::new(
            WinHttpConnect(session.0, PCWSTR(host.as_ptr()), port, 0),
            "connect",
        )?;
        let path = wide(path);
        let request = Handle::new(
            WinHttpOpenRequest(
                connection.0,
                w!("GET"),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ),
            "request",
        )?;
        WinHttpSendRequest(request.0, headers.as_deref(), None, 0, 0, 0)
            .map_err(|e| format!("send: {e}"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|e| format!("receive: {e}"))?;
        let status = number_header(&request, WINHTTP_QUERY_STATUS_CODE).unwrap_or(0);
        if status != 200 {
            return Err(format!("http status {status}"));
        }
        Ok(Exchange {
            request,
            _connection: connection,
            _session: session,
        })
    }
}

/// Fetches `url` and returns the body as text.
///
/// # Errors
///
/// Returns a short message for a non-HTTPS URL, a failed call, a status other than 200 or a body
/// over 2 MB.
pub fn get(url: &str) -> Result<String, String> {
    get_with_accept(url, None)
}

/// Like [`get`], with an `Accept` header (the GitHub API wants one).
///
/// # Errors
///
/// As [`get`]; a 404 reads exactly [`NOT_FOUND`].
pub fn get_with_accept(url: &str, accept: Option<&str>) -> Result<String, String> {
    let exchange = open(url, accept)?;
    read_body(&exchange.request)
}

/// A numeric response header, if the response carries it.
fn number_header(request: &Handle, header: u32) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    // SAFETY: `value` is a 4-byte buffer and `size` says so; the response has been received.
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            header | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&raw mut value).cast()),
            &raw mut size,
            std::ptr::null_mut(),
        )
    }
    .ok()?;
    Some(value)
}

/// Reads one chunk into `chunk`; 0 means the body ended.
fn read_chunk(request: &Handle, chunk: &mut [u8]) -> Result<usize, String> {
    let mut read = 0u32;
    // SAFETY: the buffer is `chunk.len()` bytes and `read` receives the count written.
    unsafe {
        WinHttpReadData(
            request.0,
            chunk.as_mut_ptr().cast(),
            u32::try_from(chunk.len()).unwrap_or(u32::MAX),
            &raw mut read,
        )
    }
    .map_err(|e| format!("read: {e}"))?;
    Ok(read as usize)
}

/// Reads the whole body of a request whose response has been received.
fn read_body(request: &Handle) -> Result<String, String> {
    let mut body = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = read_chunk(request, &mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > MAX_BODY {
            return Err("body too large".to_string());
        }
    }
    String::from_utf8(body).map_err(|_| "body is not UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::{accept_header, split};

    /// `WinHttpSendRequest` takes the header length from the slice, so a trailing NUL would be
    /// sent as part of the header and the call fails with `ERROR_INVALID_PARAMETER` (measured:
    /// every update check on this PC, 2026-11-25).
    #[test]
    fn the_accept_header_carries_no_nul() {
        let header = accept_header("application/vnd.github+json");
        assert!(!header.contains(&0));
        assert_eq!(
            String::from_utf16(&header).ok().as_deref(),
            Some("Accept: application/vnd.github+json\r\n")
        );
    }

    /// Live: the request the update check sends. Run by hand with `--ignored`.
    #[test]
    #[ignore = "network"]
    fn github_latest_answers_with_the_accept_header() {
        let body = super::get_with_accept(
            "https://api.github.com/repos/berkeerdo/Plith/releases/latest",
            Some("application/vnd.github+json"),
        );
        assert!(body.is_ok_and(|text| text.contains("\"tag_name\"")));
    }

    #[test]
    fn only_https_is_accepted() {
        assert!(split("http://example.com/").is_err());
        assert_eq!(split("https://a.b/c?d=1"), Ok(("a.b", 443, "/c?d=1")));
        assert_eq!(split("https://a.b:8443"), Ok(("a.b", 8443, "/")));
    }
}
