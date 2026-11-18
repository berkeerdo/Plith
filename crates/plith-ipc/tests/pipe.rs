//! Real pipe server and client in one process, on a private pipe name per test.
#![cfg(windows)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use plith_ipc::{
    BindError, DecodeError, Flow, Message, Meter, OsdLink, OsdServer, Rejection, ServerEvent,
    ServerTrust, Stopper, encode,
};
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_BUSY, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_MODE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    WriteFile,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe};
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

/// A link that accepts a server at its own level: the test server runs in this process.
fn dev_link(name: String) -> OsdLink {
    OsdLink::start_with(name, ServerTrust::DevSameUser)
}

fn level(percent: u8) -> Message {
    Message::Level {
        meter: Meter::Volume,
        percent,
        muted: false,
        label: String::new(),
        muted_word: String::new(),
    }
}

/// Writes raw bytes as one pipe message each, bypassing the client, then closes.
fn write_raw(name: &str, messages: &[&[u8]]) {
    let handle = open_raw(name);
    for bytes in messages {
        // SAFETY: the handle is open and the buffer outlives the call.
        unsafe { WriteFile(handle, Some(bytes), None, None) }.expect("write");
    }
    // SAFETY: the handle is closed once.
    unsafe {
        let _ = CloseHandle(handle);
    }
}

/// This process's own `Hello`, the admission ticket of a raw client.
fn own_hello() -> Vec<u8> {
    encode(&Message::Hello {
        client_pid: std::process::id(),
    })
    .unwrap()
}

/// Opens the pipe as a raw client, waiting while the single instance is busy.
fn open_raw(name: &str) -> HANDLE {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // The server needs a moment to listen again after it disconnects a client.
    let start = Instant::now();
    loop {
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
            Ok(handle) => return handle,
            Err(e) => {
                assert!(start.elapsed() < WAIT, "open pipe: {e}");
                assert_eq!(e.code(), ERROR_PIPE_BUSY.to_hresult(), "open pipe: {e}");
                // SAFETY: the name is NUL-terminated and outlives the call.
                let _ = unsafe { WaitNamedPipeW(PCWSTR(wide.as_ptr()), 200) };
            }
        }
    }
}

#[test]
fn client_messages_reach_the_server_in_order() {
    let name = unique_name();
    let server = Running::start(&name);
    let link = dev_link(name);
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
        let link = dev_link(name.clone());
        assert_eq!(
            server.next_event(),
            ServerEvent::Connected {
                pid: std::process::id()
            }
        );
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
        let link = dev_link(name.clone());
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
    let link = dev_link(name.clone());
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
    let link = dev_link(name.clone());
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
    write_raw(&name, &[&own_hello(), &big, &ok]);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
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
    write_raw(&name, &[&own_hello(), b"XXjunk", &behind]);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
    assert_eq!(server.next(), Err(DecodeError::Magic));
    write_raw(&name, &[&own_hello(), &ok]);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
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
    let link = dev_link(unique_name());
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
    let link = dev_link(name);
    let long = Message::Level {
        meter: Meter::Volume,
        percent: 1,
        muted: false,
        label: "x".repeat(60),
        muted_word: String::new(),
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

/// Plays a protocol 1 helper that keeps serving: it accepts a client, reads its first frame and,
/// like the old decoder with a frame of another version (a fatal error), disconnects; then it
/// listens again. Returns how many clients it rejected.
fn old_helper(name: String, stop: Arc<AtomicBool>) -> JoinHandle<u32> {
    thread::spawn(move || {
        let mut rejected = 0;
        while !stop.load(Ordering::SeqCst) {
            let pipe = create_stuck_pipe(&name);
            // SAFETY: the handle is valid; a client that connected first reports an error,
            // which is fine, its data is still readable.
            let _ = unsafe { ConnectNamedPipe(pipe, None) };
            let mut buffer = [0u8; 256];
            let mut read = 0u32;
            // SAFETY: the buffer and the count outlive the call.
            let got = unsafe { ReadFile(pipe, Some(&mut buffer), Some(&raw mut read), None) };
            // The version 1 decoder accepts only a version byte of 1 (written as a literal).
            if got.is_ok() && read >= 3 && buffer[2] != 1 {
                rejected += 1;
            }
            // SAFETY: the handle is disconnected and closed once.
            unsafe {
                let _ = DisconnectNamedPipe(pipe);
                let _ = CloseHandle(pipe);
            }
        }
        rejected
    })
}

/// Connects and closes a raw client until the helper thread has ended, to free its accept.
fn poke_until_finished(name: &str, helper: &JoinHandle<u32>) {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    while !helper.is_finished() {
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
        if let Ok(handle) = opened {
            // SAFETY: the handle is closed once.
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn an_old_helper_fails_closed() {
    let name = unique_name();
    let stop = Arc::new(AtomicBool::new(false));
    let helper = old_helper(name.clone(), Arc::clone(&stop));
    let link = dev_link(name.clone());
    // The helper keeps accepting and rejecting for the whole window: the link may reconnect
    // again and again, but it must never read as up (no flyout suppression behind nothing).
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        assert!(!link.is_connected(), "an old helper must never read as up");
        link.send(level(9)).unwrap();
        thread::sleep(Duration::from_millis(10));
    }
    assert!(link.last_error().is_some(), "the failure is reported");
    stop.store(true, Ordering::SeqCst);
    drop(link);
    poke_until_finished(&name, &helper);
    let rejected = helper.join().expect("the old helper thread");
    assert!(rejected >= 2, "it was tried again and again: {rejected}");
}

#[test]
fn a_real_server_is_reported_up_after_admission() {
    let name = unique_name();
    let server = Running::start(&name);
    let link = dev_link(name);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
    assert!(
        eventually(|| link.is_connected()),
        "up once the admission ping wrote"
    );
    assert_eq!(link.last_error(), None);
    link.send(level(5)).unwrap();
    assert_eq!(server.next(), Ok(level(5)));
}

#[test]
fn the_default_link_refuses_a_server_at_its_own_level() {
    let name = unique_name();
    let server = Running::start(&name);
    let link = OsdLink::start(name);
    assert!(
        eventually(|| link
            .last_error()
            .is_some_and(|error| error.contains("not trusted"))),
        "a same-level server is refused: {:?}",
        link.last_error()
    );
    assert!(!link.is_connected());
    assert!(
        server.quiet_for(Duration::from_millis(300)),
        "no Hello reaches it"
    );
}

/// The next `Rejected` event, skipping nothing else: any other event fails the test.
fn next_rejection(server: &Running) -> Rejection {
    match server.next_event() {
        ServerEvent::Rejected(why) => why,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn wrong_hello_pid_is_rejected() {
    let name = unique_name();
    let server = Running::start(&name);
    let liar = encode(&Message::Hello { client_pid: 4 }).unwrap();
    let hide = encode(&Message::Hide).unwrap();
    write_raw(&name, &[&liar, &hide]);
    assert_eq!(
        next_rejection(&server),
        Rejection::PidMismatch {
            kernel: std::process::id(),
            hello: 4
        }
    );
    assert!(
        server.quiet_for(Duration::from_millis(200)),
        "nothing from a rejected client is delivered"
    );
}

#[test]
fn a_first_message_other_than_hello_is_rejected() {
    let name = unique_name();
    let server = Running::start(&name);
    write_raw(&name, &[&encode(&Message::Hide).unwrap()]);
    assert_eq!(next_rejection(&server), Rejection::NotHello);
}

#[test]
fn silent_client_is_dropped_and_the_next_one_served() {
    let name = unique_name();
    let server = Running::start(&name);
    let silent = open_raw(&name);
    let start = Instant::now();
    assert_eq!(next_rejection(&server), Rejection::Timeout);
    let waited = start.elapsed();
    assert!(
        waited >= Duration::from_millis(900) && waited < Duration::from_millis(2000),
        "dropped after {waited:?}"
    );
    let link = dev_link(name);
    assert_eq!(
        server.next_event(),
        ServerEvent::Connected {
            pid: std::process::id()
        }
    );
    link.send(level(3)).unwrap();
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
    assert_eq!(server.next(), Ok(level(3)));
    // SAFETY: the handle is closed once.
    unsafe {
        let _ = CloseHandle(silent);
    }
}

#[test]
fn a_probe_that_only_opens_and_closes_is_not_a_connection() {
    let name = unique_name();
    let server = Running::start(&name);
    write_raw(&name, &[]);
    assert_eq!(next_rejection(&server), Rejection::ClosedBeforeHello);
}

#[test]
fn a_panicking_callback_unwinds_out_of_serve() {
    let name = unique_name();
    let server = OsdServer::bind(&name).expect("bind");
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            server.serve(|event| {
                if matches!(event, ServerEvent::Connected { .. }) {
                    std::panic::resume_unwind(Box::new("callback failure"));
                }
                Flow::Continue
            })
        }));
        let _ = done.send(outcome.is_err());
    });
    let _link = dev_link(name);
    let panicked = finished
        .recv_timeout(Duration::from_secs(3))
        .expect("serve must unwind instead of hanging on its clock thread");
    assert!(panicked, "the panic reaches the caller");
}

#[test]
fn silent_admitted_client_is_dropped_after_the_idle_timeout() {
    let name = unique_name();
    let mut server = OsdServer::bind(&name).expect("bind");
    // Longer than the client's admission window, which the real link waits out before it writes.
    server.set_idle_timeout(Duration::from_millis(900));
    let stopper = server.stopper();
    let (tx, events) = mpsc::channel();
    let thread = thread::spawn(move || {
        let _ = server.serve(|event| {
            let _ = tx.send(event);
            Flow::Continue
        });
    });
    let silent = open_raw(&name);
    // SAFETY: the handle is open and the buffer outlives the call.
    unsafe { WriteFile(silent, Some(&own_hello()), None, None) }.expect("hello");
    let next = || events.recv_timeout(WAIT).expect("an event in time");
    assert!(matches!(next(), ServerEvent::Connected { .. }));
    assert!(matches!(
        next(),
        ServerEvent::Message(Ok(Message::Hello { .. }))
    ));
    let start = Instant::now();
    assert_eq!(
        next(),
        ServerEvent::Disconnected,
        "the idle client is dropped"
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    // The single instance is free again: a real client is served.
    let link = dev_link(name);
    assert!(matches!(next(), ServerEvent::Connected { .. }));
    link.send(Message::Hide).unwrap();
    assert!(matches!(
        next(),
        ServerEvent::Message(Ok(Message::Hello { .. }))
    ));
    assert_eq!(next(), ServerEvent::Message(Ok(Message::Hide)));
    drop(link);
    stopper.stop();
    thread.join().expect("serve thread");
    // SAFETY: the handle is closed once.
    unsafe {
        let _ = CloseHandle(silent);
    }
}

#[test]
fn ping_is_swallowed_and_keeps_the_client() {
    let name = unique_name();
    let server = Running::start(&name);
    let ping = encode(&Message::Ping).unwrap();
    let hide = encode(&Message::Hide).unwrap();
    write_raw(&name, &[&own_hello(), &ping, &hide]);
    assert!(matches!(server.next(), Ok(Message::Hello { .. })));
    assert_eq!(
        server.next(),
        Ok(Message::Hide),
        "Ping never reaches the callback"
    );
}
