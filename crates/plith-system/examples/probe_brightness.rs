//! Read-only probe: prints every controllable display with its level, then 5 s of change
//! events (a built-in panel announces them; an external monitor never does). It never calls
//! `set_percent`, so nothing is written to any display.

#[cfg(windows)]
fn main() {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant};

    use plith_system::{BrightnessService, DeviceLevel, Status};

    fn print_levels(levels: &[DeviceLevel]) {
        if levels.is_empty() {
            println!("  (no controllable display)");
        }
        for level in levels {
            println!("  {} | {} | {}%", level.id, level.name, level.percent);
        }
    }

    let service = BrightnessService::new();
    let started = Instant::now();
    let status = service.start();
    println!(
        "status: {status:?} (discovery took {} ms)",
        started.elapsed().as_millis()
    );
    if matches!(status, Status::Unavailable(_)) {
        return;
    }
    let events = service.subscribe();
    println!("displays:");
    print_levels(&service.snapshot());

    println!("watching for 5 s (read only)");
    let watch = Instant::now();
    let mut count = 0;
    while let Some(left) = Duration::from_secs(5).checked_sub(watch.elapsed()) {
        match events.recv_timeout(left) {
            Ok(levels) => {
                count += 1;
                println!("[{:4.1}s] change:", watch.elapsed().as_secs_f64());
                print_levels(&levels);
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
    println!("display brightness is Windows only");
}
