use super::*;

#[test]
fn messages_read_as_one_log_word_each() {
    assert_eq!(describe(&Message::Hello { client_pid: 42 }), "hello pid 42");
    assert_eq!(describe(&Message::Hide), "hide");
    assert_eq!(describe(&Message::Bye), "bye");
    let level = Message::Level {
        meter: Meter::Brightness,
        percent: 50,
        muted: false,
        label: String::from("Display"),
    };
    assert_eq!(
        describe(&level),
        "level brightness 50 muted=false label=\"Display\""
    );
}

#[test]
fn a_full_channel_waits_for_room_and_drops_nothing() {
    let (events, inbox) = sync_channel(1);
    let log = Arc::new(Log::closed());
    let mut served = Served::default();
    let level = Message::Level {
        meter: Meter::Volume,
        percent: 7,
        muted: false,
        label: String::new(),
    };
    let sent = vec![level, Message::Hide, Message::Bye];
    let expected = sent.clone();
    let sender = std::thread::spawn(move || {
        let flows: Vec<Flow> = sent
            .into_iter()
            .map(|message| served.on(ServerEvent::Message(Ok(message)), &events, &log))
            .collect();
        (flows, served, events, log)
    });
    // Let the sender meet the full channel before any room is made.
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut got = Vec::new();
    while got.len() < expected.len() {
        match inbox.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Event::Message(message)) => got.push(message),
            Ok(other) => panic!("unexpected event {other:?}"),
            Err(error) => panic!("a message was dropped: {error}"),
        }
    }
    assert_eq!(got, expected, "every message arrives, in order");
    let (flows, mut served, events, log) = sender.join().unwrap();
    assert!(flows.iter().all(|flow| *flow == Flow::Continue));
    drop(inbox);
    assert_eq!(
        served.on(ServerEvent::Message(Ok(Message::Hide)), &events, &log),
        Flow::Stop,
        "a closed render channel stops the server"
    );
}

#[test]
fn decode_errors_are_remembered_per_kind() {
    let (events, _inbox) = sync_channel(4);
    let log = Arc::new(Log::closed());
    let mut served = Served::default();
    served.on(
        ServerEvent::Message(Err(DecodeError::Version(9))),
        &events,
        &log,
    );
    served.on(
        ServerEvent::Message(Err(DecodeError::Version(8))),
        &events,
        &log,
    );
    served.on(ServerEvent::Message(Err(DecodeError::Short)), &events, &log);
    assert_eq!(served.errors.len(), 2);
}

#[test]
fn connection_events_are_forwarded_or_logged() {
    let (events, inbox) = sync_channel(4);
    let log = Arc::new(Log::closed());
    let mut served = Served::default();
    assert_eq!(
        served.on(
            ServerEvent::Connected {
                pid: std::process::id()
            },
            &events,
            &log
        ),
        Flow::Continue
    );
    assert!(served.watcher.is_some(), "a connect starts a watcher");
    assert!(
        inbox.try_recv().is_err(),
        "a connect needs nothing from the renderer"
    );
    assert_eq!(
        served.on(ServerEvent::Disconnected, &events, &log),
        Flow::Continue
    );
    assert!(matches!(inbox.try_recv(), Ok(Event::Disconnected)));
    assert!(served.watcher.is_none(), "a disconnect cancels the watcher");
    assert!(
        inbox.try_recv().is_err(),
        "a cancelled watcher sends nothing"
    );
    drop(inbox);
    assert_eq!(
        served.on(ServerEvent::Disconnected, &events, &log),
        Flow::Stop
    );
}

#[test]
fn hello_waits_for_room_instead_of_being_dropped() {
    let (events, inbox) = sync_channel(1);
    let log = Arc::new(Log::closed());
    let mut served = Served::default();
    served.on(ServerEvent::Message(Ok(Message::Hide)), &events, &log);
    let sender = std::thread::spawn(move || {
        // pid 0 cannot be opened, so the watcher reports it gone at once; that is fine here.
        served.on(
            ServerEvent::Message(Ok(Message::Hello { client_pid: 0 })),
            &events,
            &log,
        )
    });
    // Let the sender meet the full channel before any room is made.
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(matches!(inbox.recv(), Ok(Event::Message(Message::Hide))));
    let mut got_hello = false;
    while let Ok(event) = inbox.recv_timeout(std::time::Duration::from_secs(2)) {
        got_hello |= matches!(event, Event::Message(Message::Hello { .. }));
    }
    assert!(got_hello, "a full channel must not drop Hello");
    assert_eq!(sender.join().unwrap(), Flow::Continue);
}

#[test]
fn placement_follows_the_monitor_size_and_scale() {
    // The review's case: a 3840 wide desktop at 150 %, then a game at 1920 x 1080 and 100 %.
    let desktop = RectPx {
        x: 0,
        y: 0,
        width: 3840,
        height: 2160,
    };
    let game = RectPx {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let before = placement(desktop, 1.5);
    let after = placement(game, 1.0);
    assert!((before.scale - 1.5).abs() < f64::EPSILON);
    assert!((after.scale - 1.0).abs() < f64::EPSILON);
    assert_eq!(before.surface, band::top_centre(desktop, 1.5, 300.0, 40.0));
    assert_eq!(
        after.surface,
        RectPx {
            x: (1920 - 300) / 2,
            y: 0,
            width: 300,
            height: 40,
        },
        "centred on the new width, at the new scale"
    );
    assert!(
        after.surface.x + after.surface.width <= game.width,
        "never cut off"
    );
}

#[test]
fn a_panic_on_the_server_thread_asks_the_renderer_to_quit() {
    let (events, inbox) = sync_channel(4);
    let worker = std::thread::spawn(move || {
        let _guard = QuitOnPanic(&events);
        std::panic::resume_unwind(Box::new("server failure"));
    });
    assert!(worker.join().is_err(), "the thread panicked");
    assert!(matches!(inbox.try_recv(), Ok(Event::Quit)));
}

#[test]
fn a_normal_server_exit_sends_no_quit_from_the_guard() {
    let (events, inbox) = sync_channel(4);
    drop(QuitOnPanic(&events));
    assert!(inbox.try_recv().is_err());
}

#[test]
fn a_panic_inside_a_real_serve_reaches_the_quit_guard() {
    let name = format!(r"\\.\pipe\plith-osd-apptest-{}", std::process::id());
    let server = OsdServer::bind(&name).expect("bind");
    let (events, inbox) = sync_channel(4);
    let worker = std::thread::spawn(move || {
        let _guard = QuitOnPanic(&events);
        let _ = server.serve(|event| {
            if matches!(event, ServerEvent::Connected { .. }) {
                std::panic::resume_unwind(Box::new("callback failure"));
            }
            Flow::Continue
        });
    });
    let _link = plith_ipc::OsdLink::start_with(name, plith_ipc::ServerTrust::DevSameUser);
    match inbox.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(Event::Quit) => {}
        other => panic!("the helper must be told to quit, got {other:?}"),
    }
    assert!(worker.join().is_err(), "the server thread panicked");
}
