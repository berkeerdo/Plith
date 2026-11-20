use super::*;
use plith_core::settings::{IdleLook, parse};
use std::path::Path;
use std::sync::atomic::AtomicBool;
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

#[test]
fn saver_latest_wins() {
    let path = folder("latest").join("settings.ini");
    let saver = quiet(path.clone());
    saver.save(with_look(IdleLook::Clock));
    saver.save(with_look(IdleLook::Plain));
    saver.save(with_look(IdleLook::Hidden));
    saver.flush_and_join();
    assert_eq!(read_look(&path), IdleLook::Hidden);
}

#[test]
fn saver_flush_writes_pending() {
    let path = folder("flush").join("settings.ini");
    let saver = quiet(path.clone());
    saver.save(with_look(IdleLook::Playing));
    saver.flush_and_join();
    assert_eq!(read_look(&path), IdleLook::Playing);
    assert!(!folder("flush").join("settings.ini.tmp").exists());
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
    let deadline = Instant::now() + Duration::from_secs(5);
    while !saver.failed() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(saver.failed());
    assert!(woke.load(Ordering::Acquire), "the UI is woken to show it");
    assert!(reported.load(Ordering::Acquire), "the failure is logged");
    saver.flush_and_join();
}

#[test]
fn a_later_good_save_clears_the_failure() {
    let path = folder("recover").join("settings.ini");
    let saver = quiet(path.clone());
    saver.failed.store(true, Ordering::Release);
    saver.save(with_look(IdleLook::Clock));
    let deadline = Instant::now() + Duration::from_secs(5);
    while saver.failed() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!saver.failed());
    saver.flush_and_join();
}
