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
fn a_full_channel_drops_and_keeps_serving() {
    let (events, inbox) = sync_channel(1);
    let log = Arc::new(Log::closed());
    let mut served = Served::default();
    assert_eq!(
        served.on(ServerEvent::Message(Ok(Message::Hide)), &events, &log),
        Flow::Continue
    );
    assert_eq!(
        served.on(ServerEvent::Message(Ok(Message::Bye)), &events, &log),
        Flow::Continue
    );
    assert!(served.dropped, "the second message found the channel full");
    assert!(matches!(
        inbox.try_recv(),
        Ok(Event::Message(Message::Hide))
    ));
    drop(inbox);
    assert_eq!(
        served.on(ServerEvent::Message(Ok(Message::Hide)), &events, &log),
        Flow::Stop
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
