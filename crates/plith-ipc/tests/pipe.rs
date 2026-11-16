//! Real pipe server and client in one process, on a private pipe name per test.
#![cfg(windows)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use plith_ipc::{
    BindError, DecodeError, Flow, Message, Meter, OsdLink, OsdServer, Stopper, encode,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_MODE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    WriteFile,
};
use windows::core::PCWSTR;

type Event = Result<Message, DecodeError>;

const WAIT: Duration = Duration::from_millis(2500);

fn unique_name() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    format!(r"\\.\pipe\plith-osd-test-{}-{n}", std::process::id())
}

struct Running {
    stopper: Stopper,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
}

impl Running {
    fn start(name: &str) -> Self {
        let server = OsdServer::bind(name).expect("bind");
        let stopper = server.stopper();
        let (tx, events) = mpsc::channel();
        let thread = thread::spawn(move || {
            let outcome = server.serve(|event| {
                let _ = tx.send(event);
                Flow::Continue
            });
            if let Err(error) = outcome {
                eprintln!("serve failed: {error}");
            }
        });
        Self {
            stopper,
            events,
            thread: Some(thread),
        }
    }

    fn next(&self) -> Event {
        self.events.recv_timeout(WAIT).expect("an event in time")
    }

    fn shut_down(&mut self) {
        self.stopper.stop();
        if let Some(thread) = self.thread.take() {
            thread.join().expect("serve thread");
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shut_down();
    }
}

fn level(percent: u8) -> Message {
    Message::Level {
        meter: Meter::Volume,
        percent,
        muted: false,
        label: String::new(),
    }
}

/// Writes raw bytes as one pipe message, bypassing the client.
fn write_raw(name: &str, messages: &[&[u8]]) {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the name is NUL-terminated and outlives the call.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            None,
        )
    }
    .expect("open pipe");
    for bytes in messages {
        // SAFETY: the handle is open and the buffer outlives the call.
        unsafe { WriteFile(handle, Some(bytes), None, None) }.expect("write");
    }
    // SAFETY: the handle is closed once.
    unsafe {
        let _ = CloseHandle(handle);
    }
}

#[test]
fn client_messages_reach_the_server_in_order() {
    let name = unique_name();
    let server = Running::start(&name);
    let link = OsdLink::start(name);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
    for percent in [10, 20, 30] {
        link.send(level(percent)).unwrap();
        assert_eq!(server.next(), Ok(level(percent)));
    }
    link.send(Message::Hide).unwrap();
    assert_eq!(server.next(), Ok(Message::Hide));
}

#[test]
fn second_bind_reports_already_running() {
    let name = unique_name();
    let _first = OsdServer::bind(&name).expect("first bind");
    assert!(matches!(
        OsdServer::bind(&name),
        Err(BindError::AlreadyRunning)
    ));
}

#[test]
fn hello_comes_first_on_every_connection() {
    let name = unique_name();
    let server = Running::start(&name);
    for _ in 0..2 {
        let link = OsdLink::start(name.clone());
        let Ok(Message::Hello { client_pid }) = server.next() else {
            panic!("the first message must be Hello");
        };
        assert_eq!(client_pid, std::process::id());
        link.send(level(5)).unwrap();
        assert_eq!(server.next(), Ok(level(5)));
        drop(link);
        assert_eq!(server.next(), Ok(Message::Bye));
    }
}

#[test]
fn client_reconnects_after_server_restart() {
    let name = unique_name();
    let mut first = Running::start(&name);
    let link = OsdLink::start(name.clone());
    assert!(matches!(first.next(), Ok(Message::Hello { .. })));
    first.shut_down();
    drop(first);

    let second = Running::start(&name);
    let start = Instant::now();
    link.send(level(42)).unwrap();
    assert!(matches!(second.next(), Ok(Message::Hello { .. })));
    assert_eq!(second.next(), Ok(level(42)));
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn oversized_message_is_reported_and_the_next_one_still_arrives() {
    let name = unique_name();
    let server = Running::start(&name);
    let big = vec![b'x'; 1000];
    let ok = encode(&Message::Hide).unwrap();
    write_raw(&name, &[&big, &ok]);
    assert_eq!(server.next(), Err(DecodeError::Trailing));
    assert_eq!(server.next(), Ok(Message::Hide));
}

#[test]
fn fatal_decode_error_disconnects_and_the_next_client_is_served() {
    let name = unique_name();
    let server = Running::start(&name);
    let ok = encode(&Message::Hide).unwrap();
    // The second message sits behind the fatal one and must never be delivered.
    write_raw(&name, &[b"XXjunk", &ok]);
    assert_eq!(server.next(), Err(DecodeError::Magic));
    write_raw(&name, &[&ok]);
    assert_eq!(server.next(), Ok(Message::Hide));
}

#[test]
fn stopper_ends_serve_while_idle() {
    let name = unique_name();
    let mut server = Running::start(&name);
    let start = Instant::now();
    server.shut_down();
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn dropping_the_link_without_a_server_returns_quickly() {
    let link = OsdLink::start(unique_name());
    thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    drop(link);
    assert!(start.elapsed() < Duration::from_secs(1));
}
