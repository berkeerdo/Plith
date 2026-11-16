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
        served.on(ServerEvent::Connected, &events, &log),
        Flow::Continue
    );
    assert!(
        inbox.try_recv().is_err(),
        "a connect needs nothing from the renderer"
    );
    assert_eq!(
        served.on(ServerEvent::Disconnected, &events, &log),
        Flow::Continue
    );
    assert!(matches!(inbox.try_recv(), Ok(Event::Disconnected)));
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
