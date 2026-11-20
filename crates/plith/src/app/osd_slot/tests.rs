use super::*;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(5);

/// A side that counts itself live and takes `linger` to stop.
struct FakeSide {
    live: Arc<AtomicUsize>,
    linger: Duration,
}

impl Drop for FakeSide {
    fn drop(&mut self) {
        thread::sleep(self.linger);
        self.live.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Records every start and the most sides ever live at once.
#[derive(Clone)]
struct FakeStart {
    starts: Arc<Mutex<Vec<FlyoutMode>>>,
    live: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    linger: Duration,
}

impl OsdStart for FakeStart {
    type Side = FakeSide;

    fn start(&self, mode: FlyoutMode) -> Option<FakeSide> {
        self.starts.lock().unwrap().push(mode);
        let now = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        Some(FakeSide {
            live: Arc::clone(&self.live),
            linger: self.linger,
        })
    }

    fn sink(_side: &FakeSide) -> OsdSink {
        Box::new(|_| {})
    }
}

struct Rig {
    slot: OsdSlot<FakeStart>,
    fake: FakeStart,
    wakes: Receiver<()>,
}

fn rig(linger: Duration) -> Rig {
    let fake = FakeStart {
        starts: Arc::default(),
        live: Arc::default(),
        peak: Arc::default(),
        linger,
    };
    let (tx, wakes) = mpsc::channel();
    let tx = Mutex::new(tx);
    let wake: Wake = Arc::new(move || {
        let _ = tx.lock().unwrap().send(());
    });
    Rig {
        slot: OsdSlot::new(fake.clone(), wake),
        fake,
        wakes,
    }
}

impl Rig {
    fn starts(&self) -> Vec<FlyoutMode> {
        self.fake.starts.lock().unwrap().clone()
    }

    fn live(&self) -> usize {
        self.fake.live.load(Ordering::SeqCst)
    }

    /// Waits for the stopping thread's wake, then lets the slot see it, as the UI thread does.
    fn settle(&mut self) {
        self.wakes
            .recv_timeout(WAIT)
            .expect("the stop wakes the UI");
        self.slot.poll();
    }
}

#[test]
fn off_on_quickly_starts_once() {
    let mut rig = rig(Duration::from_millis(150));
    rig.slot.set(true, FlyoutMode::Off);
    assert_eq!(rig.starts().len(), 1);
    assert!(rig.slot.take_sink().is_some(), "a start hands out a sink");

    rig.slot.set(false, FlyoutMode::Off);
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.poll();
    assert_eq!(rig.starts().len(), 1, "no start while the old side stops");
    assert!(rig.slot.take_sink().is_none());

    rig.settle();
    assert_eq!(rig.starts().len(), 2, "the pending side started once");
    assert_eq!(rig.live(), 1);
    assert_eq!(rig.fake.peak.load(Ordering::SeqCst), 1, "never two live");
    assert!(rig.slot.take_sink().is_some());
    assert!(rig.slot.is_on());
}

#[test]
fn off_on_off_while_stopping_ends_off() {
    let mut rig = rig(Duration::from_millis(50));
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.set(false, FlyoutMode::Off);
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.set(false, FlyoutMode::Off);
    rig.settle();
    assert_eq!(rig.starts().len(), 1);
    assert_eq!(rig.live(), 0);
    assert!(!rig.slot.is_on());
}

#[test]
fn restart_changes_mode() {
    let mut rig = rig(Duration::from_millis(20));
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.restart(FlyoutMode::Hide);
    rig.settle();
    assert_eq!(rig.starts(), [FlyoutMode::Off, FlyoutMode::Hide]);
    assert_eq!(rig.live(), 1);
}

#[test]
fn restart_while_off_starts_nothing() {
    let mut rig = rig(Duration::ZERO);
    rig.slot.restart(FlyoutMode::Hide);
    assert!(rig.starts().is_empty());
    rig.slot.set(true, FlyoutMode::Hide);
    assert_eq!(rig.starts(), [FlyoutMode::Hide]);
}

#[test]
fn apply_restarts_only_for_a_new_mode() {
    let mut rig = rig(Duration::from_millis(20));
    rig.slot.apply(true, FlyoutMode::Off);
    rig.slot.apply(true, FlyoutMode::Off);
    assert_eq!(rig.starts().len(), 1, "the same mode keeps the side");
    rig.slot.apply(true, FlyoutMode::Hide);
    rig.settle();
    assert_eq!(rig.starts(), [FlyoutMode::Off, FlyoutMode::Hide]);
    rig.slot.apply(false, FlyoutMode::Hide);
    rig.settle();
    assert_eq!(rig.live(), 0);
}

#[test]
fn set_never_blocks() {
    let mut rig = rig(Duration::from_secs(1));
    rig.slot.set(true, FlyoutMode::Off);
    let begun = Instant::now();
    rig.slot.set(false, FlyoutMode::Off);
    let took = begun.elapsed();
    assert!(took < Duration::from_millis(50), "set(false) took {took:?}");
}

#[test]
fn shutdown_joins_a_stopping_side() {
    let mut rig = rig(Duration::from_millis(100));
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.set(false, FlyoutMode::Off);
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.shutdown();
    assert_eq!(rig.live(), 0, "the old side finished stopping");
    assert_eq!(rig.starts().len(), 1, "no pending start at shutdown");
}

#[test]
fn shutdown_stops_a_running_side() {
    let mut rig = rig(Duration::ZERO);
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.shutdown();
    assert_eq!(rig.live(), 0);
}

#[test]
fn a_running_side_keeps_its_mode_until_a_restart() {
    let mut rig = rig(Duration::from_millis(20));
    rig.slot.set(true, FlyoutMode::Off);
    rig.slot.set(true, FlyoutMode::Hide);
    assert_eq!(rig.starts(), [FlyoutMode::Off]);
    // The slot still knows the side runs in Off, so the wish for Hide restarts it.
    rig.slot.apply(true, FlyoutMode::Hide);
    rig.settle();
    assert_eq!(rig.starts(), [FlyoutMode::Off, FlyoutMode::Hide]);
}

#[test]
fn wait_until_stops_when_gone_or_at_the_limit() {
    let calls = std::cell::Cell::new(0);
    let gone = wait_until(
        || {
            calls.set(calls.get() + 1);
            calls.get() >= 3
        },
        WAIT,
        Duration::from_millis(1),
    );
    assert!(gone);
    assert_eq!(calls.get(), 3);
    let begun = Instant::now();
    assert!(!wait_until(
        || false,
        Duration::from_millis(30),
        Duration::from_millis(5)
    ));
    assert!(begun.elapsed() >= Duration::from_millis(30));
}
