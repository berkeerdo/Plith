use super::*;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

const OWN: Version = Version::new(0, 4, 0);
const NEWER: Version = Version::new(0, 5, 0);

fn release_json(tag: &str) -> String {
    let name = format!("Plith-Setup-{}.exe", tag.trim_start_matches('v'));
    let base = "https://github.com/berkeerdo/Plith/releases";
    format!(
        r#"{{"tag_name":"{tag}","html_url":"{base}/tag/{tag}","assets":[{{"name":"{name}","browser_download_url":"{base}/download/{tag}/{name}"}}]}}"#
    )
}

fn release() -> Release {
    plith_core::update::parse_release(&release_json("v0.5.0")).unwrap()
}

#[derive(Default)]
struct Fake {
    latest: Mutex<VecDeque<Result<Option<String>, String>>>,
    fetches: AtomicUsize,
    busy: AtomicBool,
    now: AtomicU64,
    launches: Mutex<VecDeque<Result<(), LaunchError>>>,
    launched: AtomicUsize,
    percents: Vec<u8>,
    dir: PathBuf,
}

impl Fake {
    fn new(name: &str) -> Arc<Self> {
        let dir = std::env::temp_dir().join(format!("plith-update-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Arc::new(Self {
            now: AtomicU64::new(1_000_000),
            dir,
            ..Self::default()
        })
    }

    fn answer(&self, reply: Result<Option<String>, String>) {
        lock(&self.latest).push_back(reply);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Lets the service own a clone while the test keeps its own handle.
struct Shared(Arc<Fake>);

impl Backend for Shared {
    fn latest(&self) -> Result<Option<String>, String> {
        self.0.fetches.fetch_add(1, Ordering::SeqCst);
        lock(&self.0.latest)
            .pop_front()
            .unwrap_or_else(|| Err("queue empty".to_string()))
    }

    fn download(
        &self,
        _url: &str,
        dest: &Path,
        progress: &mut dyn FnMut(u8),
    ) -> Result<(), String> {
        for percent in &self.0.percents {
            progress(*percent);
        }
        std::fs::write(dest, b"setup").map_err(|e| e.to_string())
    }

    fn launch(&self, _setup: &Path, _own: Version) -> Result<(), LaunchError> {
        self.0.launched.fetch_add(1, Ordering::SeqCst);
        lock(&self.0.launches).pop_front().unwrap_or(Ok(()))
    }

    fn busy(&self) -> bool {
        self.0.busy.load(Ordering::SeqCst)
    }

    fn now(&self) -> u64 {
        self.0.now.load(Ordering::SeqCst)
    }

    fn updates_dir(&self) -> Result<PathBuf, String> {
        Ok(self.0.dir.clone())
    }
}

fn settings(enabled: bool) -> UpdateSettings {
    UpdateSettings {
        enabled,
        checked: 0,
        seen: String::new(),
    }
}

/// Starts a service over `fake`, so a test can send commands and then `finish`.
fn start(
    fake: &Arc<Fake>,
    settings: UpdateSettings,
) -> (UpdateService, Receiver<UpdateEvent>, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&wakes);
    let hooks = Hooks {
        waker: Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }),
        on_failure: Box::new(|_| {}),
    };
    let (service, feed) =
        UpdateService::start_over(OWN, settings, hooks, Box::new(Shared(Arc::clone(fake))));
    (service, feed, wakes)
}

/// Stops the service (it handles every queued command first) and returns all its events.
fn finish(service: UpdateService, feed: &Receiver<UpdateEvent>) -> Vec<UpdateEvent> {
    service.stop();
    feed.try_iter().collect()
}

#[test]
fn check_reports_ready_and_notice_once() {
    let fake = Fake::new("notice");
    fake.answer(Ok(Some(release_json("v0.5.0"))));
    fake.answer(Ok(Some(release_json("v0.5.0"))));
    let (service, feed, wakes) = start(&fake, settings(false));
    service.check_now();
    service.check_now();
    let events = finish(service, &feed);
    let notices = events
        .iter()
        .filter(|e| matches!(e, UpdateEvent::Notice(_)))
        .count();
    assert_eq!(notices, 1, "{events:?}");
    assert!(events.contains(&UpdateEvent::Seen(NEWER)));
    assert!(events.contains(&UpdateEvent::Checked(1_000_000)));
    let readies = events
        .iter()
        .filter(|e| **e == UpdateEvent::Row(UpdateRow::Ready(NEWER)))
        .count();
    assert_eq!(readies, 2, "the row shows it on every check");
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        events.len(),
        "one wake per event"
    );
}

#[test]
fn already_seen_version_gives_no_notice() {
    let fake = Fake::new("seen");
    fake.answer(Ok(Some(release_json("v0.5.0"))));
    let seen = UpdateSettings {
        seen: "0.5.0".to_string(),
        ..settings(false)
    };
    let (service, feed, _) = start(&fake, seen);
    service.check_now();
    let events = finish(service, &feed);
    assert!(!events.iter().any(|e| matches!(e, UpdateEvent::Notice(_))));
    assert!(events.contains(&UpdateEvent::Row(UpdateRow::Ready(NEWER))));
}

#[test]
fn busy_user_defers_notice() {
    let fake = Fake::new("busy");
    fake.busy.store(true, Ordering::SeqCst);
    fake.answer(Ok(Some(release_json("v0.5.0"))));
    fake.answer(Ok(Some(release_json("v0.5.0"))));
    let (service, feed, _) = start(&fake, settings(false));
    service.check_now();
    // The service handles commands in order; the flag flips between the two checks only if the
    // first is done, so wait for its row.
    let first = feed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(first, UpdateEvent::Row(UpdateRow::Checking));
    while feed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap()
        != UpdateEvent::Row(UpdateRow::Ready(NEWER))
    {}
    fake.busy.store(false, Ordering::SeqCst);
    service.check_now();
    let events = finish(service, &feed);
    let notices = events
        .iter()
        .filter(|e| matches!(e, UpdateEvent::Notice(_)))
        .count();
    assert_eq!(
        notices, 1,
        "the notice came on the check after the game: {events:?}"
    );
}

#[test]
fn disabled_check_never_fetches_but_check_now_does() {
    let fake = Fake::new("disabled");
    fake.answer(Ok(None));
    let (service, feed, _) = start(&fake, settings(false));
    service.enable(false);
    assert_eq!(fake.fetches.load(Ordering::SeqCst), 0);
    service.check_now();
    let events = finish(service, &feed);
    assert_eq!(fake.fetches.load(Ordering::SeqCst), 1);
    assert!(events.contains(&UpdateEvent::Row(UpdateRow::Current)));
}

#[test]
fn no_release_yet_is_a_success_with_nothing_new() {
    let fake = Fake::new("none");
    fake.answer(Ok(None));
    let (service, feed, _) = start(&fake, settings(false));
    service.check_now();
    let events = finish(service, &feed);
    assert!(events.contains(&UpdateEvent::Checked(1_000_000)));
    assert!(events.contains(&UpdateEvent::Row(UpdateRow::Current)));
    assert!(!events.contains(&UpdateEvent::Row(UpdateRow::Failed)));
}

#[test]
fn failed_check_shows_failed_and_records_no_check() {
    let fake = Fake::new("failed");
    fake.answer(Err("offline".to_string()));
    let (service, feed, _) = start(&fake, settings(false));
    service.check_now();
    let events = finish(service, &feed);
    assert!(events.contains(&UpdateEvent::Row(UpdateRow::Failed)));
    assert!(!events.iter().any(|e| matches!(e, UpdateEvent::Checked(_))));
}

#[test]
fn older_or_equal_release_is_current() {
    let fake = Fake::new("older");
    fake.answer(Ok(Some(release_json("v0.4.0"))));
    let (service, feed, _) = start(&fake, settings(false));
    service.check_now();
    let events = finish(service, &feed);
    assert!(events.contains(&UpdateEvent::Row(UpdateRow::Current)));
    assert!(!events.iter().any(|e| matches!(e, UpdateEvent::Notice(_))));
}

#[test]
fn bad_signature_deletes_file() {
    let mut fake = Fake::new("badsig");
    Arc::get_mut(&mut fake).unwrap().percents = vec![10, 11, 12, 50, 100];
    lock(&fake.launches).push_back(Err(LaunchError::BadSignature));
    let (service, feed, _) = start(&fake, settings(false));
    service.install(release());
    let events = finish(service, &feed);
    let file = fake.dir.join("Plith-Setup-0.5.0.exe");
    assert!(!file.exists(), "the unsigned file must be deleted");
    assert_eq!(
        events.last(),
        Some(&UpdateEvent::Row(UpdateRow::BadSignature))
    );
    let shown: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            UpdateEvent::Row(UpdateRow::Downloading(n)) => Some(*n),
            _ => None,
        })
        .collect();
    assert_eq!(shown, vec![0, 10, 12, 50, 100], "at most every 2 percent");
    let _ = std::fs::remove_dir_all(&fake.dir);
}

#[test]
fn good_download_launches_and_keeps_the_file() {
    let fake = Fake::new("good");
    let (service, feed, _) = start(&fake, settings(false));
    service.install(release());
    let events = finish(service, &feed);
    assert_eq!(fake.launched.load(Ordering::SeqCst), 1);
    assert!(fake.dir.join("Plith-Setup-0.5.0.exe").exists());
    assert!(!events.contains(&UpdateEvent::Row(UpdateRow::BadSignature)));
    let _ = std::fs::remove_dir_all(&fake.dir);
}

#[test]
fn declined_installer_returns_the_row_to_ready() {
    let fake = Fake::new("declined");
    lock(&fake.launches).push_back(Err(LaunchError::Failed("cancelled".to_string())));
    let (service, feed, _) = start(&fake, settings(false));
    service.install(release());
    let events = finish(service, &feed);
    assert_eq!(
        events.last(),
        Some(&UpdateEvent::Row(UpdateRow::Ready(NEWER)))
    );
    let _ = std::fs::remove_dir_all(&fake.dir);
}

#[test]
fn install_ready_with_nothing_found_checks_then_installs() {
    let fake = Fake::new("ready-unchecked");
    fake.answer(Ok(Some(release_json("v0.5.0"))));
    let (service, feed, _) = start(&fake, settings(false));
    service.install_ready();
    let events = finish(service, &feed);
    assert_eq!(fake.fetches.load(Ordering::SeqCst), 1);
    assert_eq!(fake.launched.load(Ordering::SeqCst), 1);
    assert!(events.contains(&UpdateEvent::Row(UpdateRow::Ready(NEWER))));
    assert!(
        !events.iter().any(|e| matches!(e, UpdateEvent::Notice(_))),
        "the click is the intent; no notice on top: {events:?}"
    );
    let _ = std::fs::remove_dir_all(&fake.dir);
}

#[test]
fn install_ready_with_nothing_newer_installs_nothing() {
    let fake = Fake::new("ready-current");
    fake.answer(Ok(Some(release_json("v0.4.0"))));
    let (service, feed, _) = start(&fake, settings(false));
    service.install_ready();
    let events = finish(service, &feed);
    assert_eq!(fake.launched.load(Ordering::SeqCst), 0);
    assert_eq!(events.last(), Some(&UpdateEvent::Row(UpdateRow::Current)));
}

#[test]
fn a_refused_final_path_fails_instead_of_offering_the_same_download() {
    let fake = Fake::new("refused");
    lock(&fake.launches).push_back(Err(LaunchError::Refused("moved".to_string())));
    let (service, feed, _) = start(&fake, settings(false));
    service.install(release());
    let events = finish(service, &feed);
    assert_eq!(events.last(), Some(&UpdateEvent::Row(UpdateRow::Failed)));
    let _ = std::fs::remove_dir_all(&fake.dir);
}

#[test]
fn install_removes_only_older_setups_by_their_own_names() {
    let fake = Fake::new("older-files");
    std::fs::create_dir_all(&fake.dir).unwrap();
    for name in [
        "Plith-Setup-0.3.3.exe",
        "Plith-Setup-0.4.0.exe.part",
        "Plith-Setup-notes.exe",
        "other.txt",
    ] {
        std::fs::write(fake.dir.join(name), b"x").unwrap();
    }
    let (service, feed, _) = start(&fake, settings(false));
    service.install(release());
    let _ = finish(service, &feed);
    assert!(!fake.dir.join("Plith-Setup-0.3.3.exe").exists());
    assert!(!fake.dir.join("Plith-Setup-0.4.0.exe.part").exists());
    assert!(fake.dir.join("Plith-Setup-notes.exe").exists());
    assert!(fake.dir.join("other.txt").exists());
    let _ = std::fs::remove_dir_all(&fake.dir);
}

#[test]
fn updates_folder_junction_is_refused() {
    let fake = Fake::new("junction");
    let data = fake.dir.join("data");
    let elsewhere = fake.dir.join("elsewhere");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    let link = data.join("updates");
    let made = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&link)
        .arg(&elsewhere)
        .output()
        .is_ok_and(|out| out.status.success());
    if !made {
        eprintln!("no junction without privileges here, skipped");
        let _ = std::fs::remove_dir_all(&fake.dir);
        return;
    }
    let mut backend = Arc::try_unwrap(fake).ok().unwrap();
    let root = backend.dir.clone();
    backend.dir = link;
    let fake = Arc::new(backend);
    let (service, feed, _) = start(&fake, settings(false));
    service.install(release());
    let events = finish(service, &feed);
    assert_eq!(events.last(), Some(&UpdateEvent::Row(UpdateRow::Failed)));
    assert_eq!(fake.launched.load(Ordering::SeqCst), 0);
    assert!(!elsewhere.join("Plith-Setup-0.5.0.exe").exists());
    let _ = std::fs::remove_dir(&fake.dir);
    let _ = std::fs::remove_dir_all(&root);
}
