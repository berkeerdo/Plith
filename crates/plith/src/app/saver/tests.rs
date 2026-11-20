use super::*;
use plith_core::settings::{IdleLook, parse};
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn folder(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("plith-saver-{name}-{}", std::process::id()))
}

fn with_look(look: IdleLook) -> Settings {
    Settings {
        look,
        ..Settings::default()
    }
}

fn read_look(path: &Path) -> IdleLook {
    parse(&std::fs::read_to_string(path).unwrap()).0.look
}

fn quiet(path: PathBuf) -> Saver {
    Saver::start(path, Box::new(|| {}), Box::new(|_| {}))
}

/// Waits up to five seconds for `saver.failed()` to read `want`.
fn wait_failed(saver: &Saver, want: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while saver.failed() != want && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn saver_latest_wins() {
    let dir = folder("latest");
    let path = dir.join("settings.ini");
    let written = Arc::new(Mutex::new(Vec::new()));
    let worker = {
        let written = Arc::clone(&written);
        Worker {
            path: path.clone(),
            failed: Arc::new(AtomicBool::new(false)),
            wake: Box::new(|| {}),
            report: Box::new(|_| {}),
            write_file: Box::new(move |path, settings| {
                written.lock().unwrap().push(settings.look);
                save_at(path, settings)
            }),
        }
    };
    // The worker waits at the gate until all three saves are queued.
    let (open, gate) = mpsc::channel();
    let saver = Saver::launch(worker, Some(gate));
    saver.save(with_look(IdleLook::Clock));
    saver.save(with_look(IdleLook::Plain));
    saver.save(with_look(IdleLook::Hidden));
    open.send(()).unwrap();
    saver.flush_and_join();
    assert_eq!(
        *written.lock().unwrap(),
        vec![IdleLook::Hidden],
        "one write"
    );
    assert_eq!(read_look(&path), IdleLook::Hidden);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saver_flush_writes_pending() {
    let dir = folder("flush");
    let path = dir.join("settings.ini");
    let saver = quiet(path.clone());
    saver.save(with_look(IdleLook::Playing));
    saver.flush_and_join();
    assert_eq!(read_look(&path), IdleLook::Playing);
    assert!(!dir.join("settings.ini.tmp").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saver_reports_failure() {
    let dir = folder("fail");
    std::fs::create_dir_all(&dir).unwrap();
    // A file where the folder should be: the folder cannot be created, so the save fails.
    let blocker = dir.join("not-a-folder");
    std::fs::write(&blocker, "x").unwrap();
    let woke = Arc::new(AtomicBool::new(false));
    let reported = Arc::new(AtomicBool::new(false));
    let saver = {
        let woke = Arc::clone(&woke);
        let reported = Arc::clone(&reported);
        Saver::start(
            blocker.join("settings.ini"),
            Box::new(move || woke.store(true, Ordering::Release)),
            Box::new(move |_| reported.store(true, Ordering::Release)),
        )
    };
    assert!(!saver.failed());
    saver.save(Settings::default());
    wait_failed(&saver, true);
    assert!(saver.failed());
    assert!(woke.load(Ordering::Acquire), "the UI is woken to show it");
    assert!(reported.load(Ordering::Acquire), "the failure is logged");
    saver.flush_and_join();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_later_good_save_clears_the_failure() {
    let dir = folder("recover");
    let saver = quiet(dir.join("settings.ini"));
    saver.failed.store(true, Ordering::Release);
    saver.save(with_look(IdleLook::Clock));
    wait_failed(&saver, false);
    assert!(!saver.failed());
    saver.flush_and_join();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_panicking_worker_marks_the_saver_failed() {
    let woke = Arc::new(AtomicBool::new(false));
    let worker = {
        let woke = Arc::clone(&woke);
        Worker {
            path: folder("panic").join("settings.ini"),
            failed: Arc::new(AtomicBool::new(false)),
            wake: Box::new(move || woke.store(true, Ordering::Release)),
            report: Box::new(|_| {}),
            write_file: Box::new(|_, _| panic!("disk on fire")),
        }
    };
    let saver = Saver::launch(worker, None);
    saver.save(Settings::default());
    wait_failed(&saver, true);
    assert!(saver.failed());
    assert!(woke.load(Ordering::Acquire));
    // The worker is gone: a later save cannot be sent and stays failed.
    saver.save(Settings::default());
    assert!(saver.failed());
    saver.flush_and_join();
}
