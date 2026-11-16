//! Real pipe server and client in one process, on a private pipe name per test.
#![cfg(windows)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use plith_ipc::{
    BindError, DecodeError, Flow, Message, Meter, OsdLink, OsdServer, ServerEvent, Stopper, encode,
};
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_BUSY, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_MODE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    WriteFile,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT, WaitNamedPipeW,
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
    events: Receiver<ServerEvent>,
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

    /// The next server event, connection events included.
    fn next_event(&self) -> ServerEvent {
        self.events.recv_timeout(WAIT).expect("an event in time")
    }

    /// The next message (or decode error), skipping connection events.
    fn next(&self) -> Event {
        loop {
            if let ServerEvent::Message(event) = self.next_event() {
                return event;
            }
        }
    }

    /// True when no message (or decode error) arrives within `time`.
    fn quiet_for(&self, time: Duration) -> bool {
        let deadline = Instant::now() + time;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(ServerEvent::Message(_)) => return false,
                Ok(_) => {}
                Err(_) => return true,
            }
        }
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
    // The server needs a moment to listen again after it disconnects a client.
    let start = Instant::now();
    let handle = loop {
        // SAFETY: the name is NUL-terminated and outlives the call.
        let opened = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        };
        match opened {
            Ok(handle) => break handle,
            Err(e) => {
                assert!(start.elapsed() < WAIT, "open pipe: {e}");
                assert_eq!(e.code(), ERROR_PIPE_BUSY.to_hresult(), "open pipe: {e}");
                // SAFETY: the name is NUL-terminated and outlives the call.
                let _ = unsafe { WaitNamedPipeW(PCWSTR(wide.as_ptr()), 200) };
            }
        }
    };
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
fn each_connection_is_framed_by_connected_and_disconnected() {
    let name = unique_name();
    let server = Running::start(&name);
    for _ in 0..2 {
        let link = OsdLink::start(name.clone());
        assert_eq!(server.next_event(), ServerEvent::Connected);
        assert!(matches!(
            server.next_event(),
            ServerEvent::Message(Ok(Message::Hello { .. }))
        ));
        link.send(Message::Hide).unwrap();
        assert_eq!(server.next_event(), ServerEvent::Message(Ok(Message::Hide)));
        drop(link);
        assert_eq!(server.next_event(), ServerEvent::Message(Ok(Message::Bye)));
        assert_eq!(server.next_event(), ServerEvent::Disconnected);
    }
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

/// Polls `check` until it holds or `WAIT` passes.
fn eventually(check: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    check()
}

#[test]
fn link_state_follows_the_server_life() {
    let name = unique_name();
    let link = OsdLink::start(name.clone());
    assert!(
        eventually(|| link.last_error().is_some()),
        "no server yet: the failed connect is reported"
    );
    assert!(!link.is_connected());

    let mut server = Running::start(&name);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
    assert!(eventually(|| link.is_connected()), "up once Hello went out");
    assert_eq!(link.last_error(), None, "a connect clears the error");

    server.shut_down();
    drop(server);
    // An idle link notices a gone server at its next send.
    link.send(level(9)).unwrap();
    assert!(eventually(|| !link.is_connected()), "down after the server");
    let error = link.last_error().expect("why the link is down");
    assert!(
        error.starts_with("write:") || error.starts_with("open:"),
        "{error}"
    );
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
    // level(1) sits behind the fatal message and must be discarded with the connection.
    let behind = encode(&level(1)).unwrap();
    write_raw(&name, &[b"XXjunk", &behind]);
    assert_eq!(server.next(), Err(DecodeError::Magic));
    write_raw(&name, &[&ok]);
    assert_eq!(server.next(), Ok(Message::Hide));
    assert!(
        server.quiet_for(Duration::from_millis(200)),
        "nothing else may be delivered"
    );
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

/// A server that accepts a client and never reads, with a tiny pipe quota.
fn create_stuck_pipe(name: &str) -> HANDLE {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the name is NUL-terminated and outlives the call.
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAGS_AND_ATTRIBUTES(0),
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            0,
            256,
            0,
            None,
        )
    };
    assert!(!handle.is_invalid());
    handle
}

#[test]
fn dropping_the_link_is_bounded_when_the_server_stops_reading() {
    let name = unique_name();
    let stuck = create_stuck_pipe(&name);
    let link = OsdLink::start(name);
    let long = Message::Level {
        meter: Meter::Volume,
        percent: 1,
        muted: false,
        label: "x".repeat(60),
    };
    // Fill the quota so the worker blocks inside WriteFile.
    for _ in 0..200 {
        link.send(long.clone()).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let start = Instant::now();
        drop(link);
        let _ = done.send(start.elapsed());
    });
    let elapsed = finished
        .recv_timeout(Duration::from_secs(3))
        .expect("drop must return");
    assert!(elapsed < Duration::from_secs(2), "drop took {elapsed:?}");
    // SAFETY: the handle is closed once.
    unsafe {
        let _ = CloseHandle(stuck);
    }
}
