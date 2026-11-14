//! Read-only probe: prints the default capture device (name, muted, volume) or
//! "no capture device". It never calls `toggle_mute` or any other write.

#[cfg(windows)]
fn main() {
    use plith_audio::endpoints::Status;
    use plith_audio::microphone::MicrophoneService;

    let service = MicrophoneService::new();
    let status = service.start();
    println!("status: {status:?}");
    if matches!(status, Status::Unavailable(_)) {
        return;
    }
    match service.snapshot() {
        Some(mic) => println!(
            "microphone: {} | muted={} volume={:.0}% | {}",
            mic.name,
            mic.muted,
            mic.volume * 100.0,
            mic.id
        ),
        None => println!("no capture device"),
    }
    service.stop();
    println!("stopped");
}

#[cfg(not(windows))]
fn main() {
    println!("Core Audio is Windows only");
}
