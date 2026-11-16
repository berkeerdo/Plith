//! The non-blocking pipe client used by the notch.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{ERROR_PIPE_BUSY, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_MODE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    WriteFile,
};
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::core::PCWSTR;

use crate::latest::Slot;
use crate::win::{OwnedHandle, wide};
use crate::{EncodeError, Message, encode};

const RETRY: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(500);

/// A link to the OSD helper that never blocks the caller.
///
/// A worker thread connects, reconnects every second and forwards the latest pending
/// message. Dropping the link sends `Bye` when connected and joins the worker.
pub struct OsdLink {
    slot: Arc<Slot<Message>>,
    worker: Option<JoinHandle<()>>,
}

impl OsdLink {
    /// Starts the worker for the pipe `name`. The pipe need not exist yet.
    #[must_use]
    pub fn start(name: String) -> Self {
        let slot = Arc::new(Slot::new());
        let worker = {
            let slot = Arc::clone(&slot);
            std::thread::spawn(move || run(&name, &slot))
        };
        Self {
            slot,
            worker: Some(worker),
        }
    }

    /// Queues a message; a message not yet sent is replaced. Never blocks.
    ///
    /// # Errors
    /// Returns [`EncodeError`] when the message is out of range.
    pub fn send(&self, message: Message) -> Result<(), EncodeError> {
        encode(&message)?;
        self.slot.put(message);
        Ok(())
    }
}

impl Drop for OsdLink {
    fn drop(&mut self) {
        self.slot.put(Message::Bye);
        self.slot.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(name: &str, slot: &Slot<Message>) {
    let wide_name = wide(name);
    let mut connection: Option<OwnedHandle> = None;
    let mut carry: Option<Message> = None;
    loop {
        if let Some(newer) = slot.take_wait(Duration::ZERO) {
            carry = Some(newer);
        }
        let Some(pipe) = connection.as_ref() else {
            if slot.is_closed() {
                return;
            }
            connection = connect(&wide_name);
            if connection.is_none()
                && let Some(newer) = slot.take_wait(RETRY)
            {
                carry = Some(newer);
            }
            continue;
        };
        if carry.is_none() {
            carry = slot.take_wait(POLL);
            if carry.is_none() {
                if slot.is_closed() {
                    return;
                }
                continue;
            }
        }
        let Some(message) = carry.take() else {
            continue;
        };
        let sent = write(pipe, &message).is_ok();
        if message == Message::Bye {
            return;
        }
        if !sent {
            connection = None;
            carry = Some(message);
        }
    }
}

/// Opens the pipe and sends `Hello`; `None` when the server is not there.
fn connect(wide_name: &[u16]) -> Option<OwnedHandle> {
    let name = PCWSTR(wide_name.as_ptr());
    let open = || {
        // SAFETY: `name` is NUL-terminated and outlives the call. The security flags let
        // the server identify this process but never impersonate it.
        unsafe {
            CreateFileW(
                name,
                GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        }
    };
    let handle = match open() {
        Ok(handle) => handle,
        Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() => {
            // SAFETY: `name` is NUL-terminated and outlives the call.
            if !unsafe { WaitNamedPipeW(name, 500) }.as_bool() {
                return None;
            }
            open().ok()?
        }
        Err(_) => return None,
    };
    let pipe = OwnedHandle(handle);
    let hello = Message::Hello {
        client_pid: std::process::id(),
    };
    write(&pipe, &hello).ok()?;
    Some(pipe)
}

fn write(pipe: &OwnedHandle, message: &Message) -> Result<(), String> {
    let bytes = encode(message).map_err(|e| e.to_string())?;
    let mut written = 0u32;
    // SAFETY: the handle is open and the buffer outlives the synchronous call.
    unsafe { WriteFile(pipe.0, Some(&bytes), Some(&raw mut written), None) }
        .map_err(|e| e.to_string())
}
