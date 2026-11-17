//! Read-only probe: lists the active render endpoints (marking the default and the attached
//! one), prints volume and mute, then prints every change for 10 s.
//!
//! It never changes anything: change the volume, mute or default device by hand while it runs.

#[cfg(windows)]
fn print_state(state: &plith_audio::endpoints::EndpointState) {
    println!("default: {:?}", state.default_id);
    for endpoint in &state.active {
        let is_default = state.default_id.as_deref() == Some(endpoint.id.as_str());
        let attached = state
            .attached
            .as_ref()
            .filter(|a| a.id.eq_ignore_ascii_case(&endpoint.id));
        let volume = attached.map_or(String::new(), |a| {
            format!(" volume={:.0}% muted={}", a.volume * 100.0, a.muted)
        });
        println!(
            "  [{}{}] {} | {} | form_factor={:?} | {}{volume}",
            if is_default { "D" } else { " " },
            if attached.is_some() { "A" } else { " " },
            endpoint.friendly_name,
            endpoint.device_name,
            endpoint.form_factor,
            endpoint.id,
        );
    }
    if state.attached.is_none() {
        println!("  (nothing attached)");
    }
}

#[cfg(windows)]
fn main() {
    use plith_audio::endpoints::{EndpointService, Status};
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant};

    let service = EndpointService::new();
    let events = service.subscribe();
    let status = service.start();
    println!("status: {status:?}");
    if matches!(status, Status::Unavailable(_)) {
        return;
    }
    let initial = service.snapshot();
    print_state(&initial);

    // Software-only pin round trip (nothing on the machine changes): attach to another
    // endpoint, then back to the default, to exercise detach and re-attach.
    let other = initial
        .active
        .iter()
        .find(|e| initial.default_id.as_deref() != Some(e.id.as_str()));
    if let Some(other) = other {
        println!("pinning {} ...", other.friendly_name);
        service.set_pin(Some(other.id.clone()));
        std::thread::sleep(Duration::from_millis(500));
        print_state(&service.snapshot());
        println!("unpinning ...");
        service.set_pin(None);
        std::thread::sleep(Duration::from_millis(500));
        print_state(&service.snapshot());
    }

    println!("watching for changes for 10 s (change volume, mute or default by hand)...");
    let started = Instant::now();
    let deadline = started + Duration::from_secs(10);
    // The subscription already holds the first state published during `start`; skip it.
    while events.try_recv().is_ok() {}
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match events.recv_timeout(left) {
            Ok(state) => {
                println!("change at t={:.1}s:", started.elapsed().as_secs_f32());
                print_state(&state);
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
    service.stop();
    println!("stopped");
}

#[cfg(not(windows))]
fn main() {
    println!("Core Audio is Windows only");
}
