//! The report pipe between the installer window (server) and the elevated worker (client).
//!
//! The window creates the pipe before it starts the worker. The pipe is inbound only: the
//! worker opens it for writing and never reads from it, so nothing the window or another
//! process sends can steer the worker. A worker that cannot open the pipe still installs; it
//! only loses its progress line.

#[cfg(test)]
mod tests;

use plith_core::setup::{Report, decode, encode};
use plith_ipc::security::{current_user_sid, setup_pipe_sddl};
use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_MORE_DATA, ERROR_PIPE_CONNECTED, FALSE, GENERIC_WRITE, GetLastError, HANDLE,
    HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAGS_AND_ATTRIBUTES,
    FILE_SHARE_NONE, OPEN_EXISTING, PIPE_ACCESS_INBOUND, ReadFile, SECURITY_ANONYMOUS,
    SECURITY_SQOS_PRESENT, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::core::PCWSTR;

/// The biggest message the server reads in one piece; a report is at most about 210 bytes.
const BUFFER: usize = 4096;

/// A kernel handle, closed on drop.
struct Handle(HANDLE);

// SAFETY: a kernel handle is a process-wide token that any thread may use or close.
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The pipe name of this window: `\\.\pipe\plith-setup-<session id>-<window pid>`.
#[must_use]
pub fn report_pipe_name() -> String {
    // SAFETY: plain query of this process id.
    let pid = unsafe { GetCurrentProcessId() };
    let mut session = 0u32;
    // SAFETY: `session` is a valid out pointer for the duration of the call.
    if unsafe { ProcessIdToSessionId(pid, &raw mut session) }.is_err() {
        session = 0;
    }
    format!("\\\\.\\pipe\\plith-setup-{session}-{pid}")
}

/// The receiving end: one inbound, message-mode instance that refuses remote clients.
///
/// A helper thread owns the pipe, accepts the one worker and decodes what it writes; garbage
/// is dropped. The thread ends when the worker closes its end or when the server is dropped
/// before any worker came.
pub struct ReportServer {
    name: String,
    reports: Receiver<Report>,
    stop: Arc<AtomicBool>,
}

impl ReportServer {
    /// Creates the pipe `name` and starts waiting for the worker.
    ///
    /// # Errors
    /// The text says why the pipe or its access rules could not be made, or that the name is
    /// already taken.
    pub fn bind(name: &str) -> Result<Self, String> {
        let pipe = create_pipe(name)?;
        let (sender, reports) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        std::thread::Builder::new()
            .name(String::from("plith-setup-reports"))
            .spawn(move || serve(&pipe, &sender, &thread_stop))
            .map_err(|error| format!("cannot start the report thread: {error}"))?;
        Ok(Self {
            name: name.to_string(),
            reports,
            stop,
        })
    }

    /// The next report, or `None` when none came within `timeout` or the worker is gone.
    #[must_use]
    pub fn next(&self, timeout: Duration) -> Option<Report> {
        self.reports.recv_timeout(timeout).ok()
    }
}

impl Drop for ReportServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // A server thread that still waits for a worker wakes up when someone connects. If a
        // worker is already connected the open fails and the thread ends when it hangs up.
        drop(open_for_writing(&self.name));
    }
}

fn create_pipe(name: &str) -> Result<Handle, String> {
    let sid = current_user_sid()?;
    let sddl = wide(&setup_pipe_sddl(&sid));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `sddl` is NUL-terminated and `descriptor` is a valid out pointer.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &raw mut descriptor,
            None,
        )
    }
    .map_err(|error| format!("security descriptor: {error}"))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: FALSE,
    };
    let wide_name = wide(name);
    // SAFETY: the name and attributes outlive the call; the descriptor is freed after it.
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide_name.as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            u32::try_from(BUFFER).unwrap_or(4096),
            0,
            Some(&raw const attributes),
        )
    };
    // SAFETY: read the error before any other call can overwrite it.
    let error = unsafe { GetLastError() };
    // SAFETY: the descriptor was allocated by the conversion call and is freed once.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
    }
    if handle.is_invalid() {
        return Err(format!(
            "CreateNamedPipeW: {}",
            windows::core::Error::from(error)
        ));
    }
    Ok(Handle(handle))
}

/// Accepts the worker and forwards what it writes, until it hangs up or the server is gone.
fn serve(pipe: &Handle, sender: &Sender<Report>, stop: &AtomicBool) {
    // SAFETY: the handle is a live pipe server handle owned by the caller.
    let connected = unsafe { ConnectNamedPipe(pipe.0, None) };
    // SAFETY: reads this thread's last error, set by the call above.
    let early = unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
    if (connected.is_err() && !early) || stop.load(Ordering::SeqCst) {
        return;
    }
    let mut buffer = [0u8; BUFFER];
    // A message longer than the buffer is dropped whole, including its remaining pieces.
    let mut skipping = false;
    loop {
        let mut read = 0u32;
        // SAFETY: `buffer` and `read` outlive the call; the pipe handle is live.
        let result = unsafe { ReadFile(pipe.0, Some(&mut buffer), Some(&raw mut read), None) };
        match result {
            Ok(()) => {
                let bytes = &buffer[..(read as usize).min(BUFFER)];
                if !skipping
                    && let Some(report) = decode(bytes)
                    && sender.send(report).is_err()
                {
                    return;
                }
                skipping = false;
            }
            Err(error) if error.code() == ERROR_MORE_DATA.to_hresult() => skipping = true,
            Err(_) => return,
        }
    }
}

/// The sending end the worker uses. Write only; it never reads from the pipe.
pub struct ReportClient(Handle);

/// Opens the pipe `name` for writing.
///
/// The open caps the server's impersonation level at anonymous: the worker is elevated, and a
/// process that created the name first must not be able to act with its token.
///
/// # Errors
/// The text says why the pipe could not be opened.
pub fn report_client(name: &str) -> Result<ReportClient, String> {
    open_for_writing(name).map(ReportClient)
}

fn open_for_writing(name: &str) -> Result<Handle, String> {
    let wide_name = wide(name);
    // SAFETY: the name outlives the call; no security attributes or template are passed.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_name.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(
                FILE_ATTRIBUTE_NORMAL.0 | SECURITY_SQOS_PRESENT.0 | SECURITY_ANONYMOUS.0,
            ),
            None,
        )
    };
    handle
        .map(Handle)
        .map_err(|error| format!("cannot open the report pipe: {error}"))
}

impl ReportClient {
    /// Writes one report as one pipe message.
    ///
    /// # Errors
    /// The text says why the write failed (usually: the window is gone).
    pub fn send(&self, report: &Report) -> Result<(), String> {
        let bytes = encode(report);
        let mut written = 0u32;
        // SAFETY: `bytes` and `written` outlive the call; the handle is live.
        unsafe { WriteFile(self.0.0, Some(&bytes), Some(&raw mut written), None) }
            .map_err(|error| format!("cannot write to the report pipe: {error}"))
    }
}
