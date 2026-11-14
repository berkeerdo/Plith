//! Read-only probe: prints every SMTC session, the current track, its timeline and
//! capabilities, then 15 s of change events. It never calls `toggle_play_pause`, `next`,
//! `previous` or `seek`.

#[cfg(windows)]
fn main() {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant, SystemTime};

    use plith_media::{MediaService, MediaSnapshot, Status, position_at};

    fn describe(snapshot: &MediaSnapshot) -> String {
        if !snapshot.has_session {
            return "no session".to_owned();
        }
        let position = snapshot.timeline.map_or_else(
            || "no timeline (live or unknown length)".to_owned(),
            |t| {
                let now = position_at(&t, snapshot.playing, SystemTime::now());
                format!(
                    "{:.1}s of {:.1}s",
                    now.as_secs_f64(),
                    t.duration.as_secs_f64()
                )
            },
        );
        format!(
            "{} | {} - {} | playing={} | {} | seek={} next={} prev={} | thumbnail={}",
            snapshot.source_id,
            snapshot.artist,
            snapshot.title,
            snapshot.playing,
            position,
            snapshot.can_seek,
            snapshot.can_next,
            snapshot.can_prev,
            snapshot
                .thumbnail
                .as_ref()
                .map_or_else(|| "none".to_owned(), |b| format!("{} bytes", b.len())),
        )
    }

    let service = MediaService::new();
    let status = service.start();
    println!("status: {status:?}");
    if matches!(status, Status::Unavailable(_)) {
        return;
    }
    let events = service.subscribe();

    let sessions = service.sessions();
    println!("sessions: {}", sessions.len());
    for session in &sessions {
        println!(
            "  {} | {:?} | playing={}",
            session.source_id, session.title, session.playing
        );
    }
    println!("current: {}", describe(&service.snapshot()));

    println!("watching for 15 s (read only)");
    let started = Instant::now();
    let mut count = 0;
    while let Some(left) = Duration::from_secs(15).checked_sub(started.elapsed()) {
        match events.recv_timeout(left) {
            Ok(snapshot) => {
                count += 1;
                println!(
                    "[{:5.1}s] {}",
                    started.elapsed().as_secs_f64(),
                    describe(&snapshot)
                );
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
    println!("{count} change events");
    service.stop();
    println!("stopped");
}

#[cfg(not(windows))]
fn main() {
    println!("SMTC is Windows only");
}
