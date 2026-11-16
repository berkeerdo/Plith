//! Unit tests for the pure brightness layer. No hardware: devices are fakes.

use std::sync::{Arc, Mutex, OnceLock};

use super::discovery::{apply_event, apply_write};
use super::run_token::RunTokens;
use super::step::{percent_to_raw, raw_to_percent};
use super::wql::{escape_wql, methods_query, panel_id, panel_name, read_query};
use super::writer::{Job, Observer, Runner};
use super::{
    BrightnessDevice, CoalescingWriter, DeviceLevel, WriteOutcome, keep_answering, step_level,
};

type Log = Arc<Mutex<Vec<(String, u8)>>>;
type Hook = Box<dyn FnMut(u8) + Send>;
type Jobs = Arc<Mutex<Vec<Job>>>;

struct Fake {
    id: String,
    level: Option<u8>,
    accept: bool,
    log: Log,
    hook: Option<Hook>,
}

impl Fake {
    fn new(id: &str, level: Option<u8>, log: &Log) -> Self {
        Self {
            id: id.to_owned(),
            level,
            accept: true,
            log: Arc::clone(log),
            hook: None,
        }
    }
}

impl BrightnessDevice for Fake {
    fn id(&self) -> &str {
        &self.id
    }
    fn read(&mut self) -> Option<u8> {
        self.level
    }
    fn write(&mut self, percent: u8) -> bool {
        self.log.lock().unwrap().push((self.id.clone(), percent));
        if let Some(hook) = self.hook.as_mut() {
            hook(percent);
        }
        self.accept
    }
}

fn new_log() -> Log {
    Arc::new(Mutex::new(Vec::new()))
}

fn written(log: &Log) -> Vec<(String, u8)> {
    log.lock().unwrap().clone()
}

fn entry(id: &str, value: u8) -> (String, u8) {
    (id.to_owned(), value)
}

/// A runner that only queues, so a test decides when the pump runs.
fn deferred() -> (Runner, Jobs) {
    let jobs: Jobs = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::clone(&jobs);
    (Box::new(move |job| queue.lock().unwrap().push(job)), jobs)
}

fn run_queued(jobs: &Jobs) {
    while let Some(job) = jobs.lock().unwrap().pop() {
        job();
    }
}

fn collecting() -> (Observer, Arc<Mutex<Vec<WriteOutcome>>>) {
    let seen: Arc<Mutex<Vec<WriteOutcome>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    (
        Box::new(move |outcome| sink.lock().unwrap().push(outcome.clone())),
        seen,
    )
}

fn one_device_writer(log: &Log) -> (CoalescingWriter, Jobs, Arc<Mutex<Vec<WriteOutcome>>>) {
    let (runner, jobs) = deferred();
    let (observer, seen) = collecting();
    let device = Fake::new("a", Some(1), log);
    (
        CoalescingWriter::new(vec![Box::new(device)], runner, observer),
        jobs,
        seen,
    )
}

#[test]
fn step_saturates_at_bounds() {
    assert_eq!(step_level(50, 5), 55);
    assert_eq!(step_level(50, -5), 45);
    assert_eq!(step_level(98, 5), 100);
    assert_eq!(step_level(100, 127), 100);
    assert_eq!(step_level(2, -5), 0);
    assert_eq!(step_level(0, -128), 0);
    assert_eq!(step_level(200, 0), 100, "an out of range input is clamped");
}

#[test]
fn raw_and_percent_convert_over_a_device_range() {
    assert_eq!(raw_to_percent(0, 30, 100), 30);
    assert_eq!(raw_to_percent(10, 60, 110), 50);
    assert_eq!(
        raw_to_percent(5, 5, 5),
        0,
        "an empty range is no division by zero"
    );
    assert_eq!(
        raw_to_percent(10, 200, 110),
        100,
        "a reading past max clamps"
    );
    assert_eq!(raw_to_percent(10, 3, 110), 0, "a reading below min clamps");
    assert_eq!(percent_to_raw(0, 100, 37), 37);
    assert_eq!(percent_to_raw(10, 110, 50), 60);
    assert_eq!(percent_to_raw(10, 10, 80), 10);
    assert_eq!(percent_to_raw(0, 30, 100), 30);
}

#[test]
fn writer_keeps_only_latest_pending() {
    let log = new_log();
    let (writer, jobs, seen) = one_device_writer(&log);
    writer.request(10);
    writer.request(20);
    writer.request(30);
    assert_eq!(
        jobs.lock().unwrap().len(),
        1,
        "one pump, not one per request"
    );
    run_queued(&jobs);
    assert_eq!(written(&log), vec![entry("a", 30)]);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn writer_clamps_to_one_hundred() {
    let log = new_log();
    let (writer, jobs, _) = one_device_writer(&log);
    writer.request(250);
    run_queued(&jobs);
    assert_eq!(written(&log), vec![entry("a", 100)]);
}

#[test]
fn writer_one_in_flight() {
    let log = new_log();
    let (runner, jobs) = deferred();
    let (observer, _) = collecting();
    let slot: Arc<OnceLock<Arc<CoalescingWriter>>> = Arc::new(OnceLock::new());
    let reentrant = Arc::clone(&slot);
    let mut device = Fake::new("a", Some(1), &log);
    // While the first write is in flight, the person asks for more values.
    device.hook = Some(Box::new(move |value| {
        if value == 10 {
            let writer = reentrant.get().unwrap();
            writer.request(40);
            writer.request(50);
        }
    }));
    let writer = Arc::new(CoalescingWriter::new(
        vec![Box::new(device)],
        runner,
        observer,
    ));
    slot.set(Arc::clone(&writer)).ok();
    writer.request(10);
    run_queued(&jobs);
    assert_eq!(
        written(&log),
        vec![entry("a", 10), entry("a", 50)],
        "the in-flight write finishes, then only the newest pending value follows"
    );
    assert!(
        jobs.lock().unwrap().is_empty(),
        "no second pump was started"
    );
    writer.request(60);
    assert_eq!(
        jobs.lock().unwrap().len(),
        1,
        "idle again: a new pump starts"
    );
}

#[test]
fn failed_device_does_not_block_others() {
    let log = new_log();
    let (runner, jobs) = deferred();
    let (observer, seen) = collecting();
    let mut refusing = Fake::new("a", Some(1), &log);
    refusing.accept = false;
    let working = Fake::new("b", Some(1), &log);
    let writer = CoalescingWriter::new(
        vec![Box::new(refusing), Box::new(working)],
        runner,
        observer,
    );
    writer.request(70);
    run_queued(&jobs);
    assert_eq!(
        written(&log),
        vec![entry("a", 70), entry("b", 70)],
        "the refusal is not retried and b still gets the value"
    );
    assert_eq!(
        *seen.lock().unwrap(),
        vec![WriteOutcome {
            value: 70,
            failed: vec!["a".to_owned()],
        }]
    );
}

fn level(id: &str, percent: u8) -> DeviceLevel {
    DeviceLevel {
        id: id.to_owned(),
        name: id.to_owned(),
        percent,
    }
}

#[test]
fn discovery_keeps_only_answering() {
    let log = new_log();
    let found = keep_answering(vec![
        Box::new(Fake::new("panel", Some(40), &log)),
        Box::new(Fake::new("dead", None, &log)),
        Box::new(Fake::new("monitor", Some(100), &log)),
    ]);
    let ids: Vec<&str> = found.devices.iter().map(|d| d.id()).collect();
    assert_eq!(
        ids,
        ["panel", "monitor"],
        "order kept, silent device dropped"
    );
    assert_eq!(
        found.levels,
        vec![level("panel", 40), level("monitor", 100)]
    );
    assert!(
        keep_answering(Vec::new()).devices.is_empty(),
        "no display is an empty state"
    );
}

#[test]
fn events_and_writes_update_the_snapshot() {
    let mut now = vec![level("a", 10), level("b", 10)];
    assert!(apply_event(&mut now, "b", 55));
    assert_eq!(now[1].percent, 55);
    assert!(!apply_event(&mut now, "b", 55), "same value is no change");
    assert!(!apply_event(&mut now, "zzz", 5), "unknown id is ignored");
    assert!(apply_write(&mut now, 80, &["a".to_owned()]));
    assert_eq!(
        (now[0].percent, now[1].percent),
        (10, 80),
        "a refused device keeps its level"
    );
}

#[test]
fn wql_escape_handles_backslash_and_quote() {
    assert_eq!(
        escape_wql(r"DISPLAY\AUS27FD\5&efad&0&UID4353_0"),
        r"DISPLAY\\AUS27FD\\5&efad&0&UID4353_0"
    );
    assert_eq!(escape_wql("it's"), r"it\'s");
    assert_eq!(
        escape_wql(r"a\'b"),
        r"a\\\'b",
        "backslash first, so the quote escape survives"
    );
}

#[test]
fn queries_carry_the_escaped_instance() {
    let name = r"DISPLAY\X\1_0";
    assert_eq!(
        read_query(name),
        r"SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE InstanceName = 'DISPLAY\\X\\1_0'"
    );
    assert_eq!(
        methods_query(name),
        r"SELECT __PATH FROM WmiMonitorBrightnessMethods WHERE InstanceName = 'DISPLAY\\X\\1_0'"
    );
    assert_eq!(panel_id(name), r"Internal panel DISPLAY\X\1_0");
}

#[test]
fn refusing_device_is_tried_again_on_the_next_value_and_blocks_no_one() {
    // Retry policy: a refusal is never retried for the same value, but the device stays in the
    // set, so the next distinct value is offered to it again; other devices are never skipped.
    let log = new_log();
    let (runner, jobs) = deferred();
    let (observer, seen) = collecting();
    let mut refusing = Fake::new("a", Some(1), &log);
    refusing.accept = false;
    let working = Fake::new("b", Some(1), &log);
    let writer = CoalescingWriter::new(
        vec![Box::new(refusing), Box::new(working)],
        runner,
        observer,
    );
    writer.request(70);
    run_queued(&jobs);
    writer.request(80);
    run_queued(&jobs);
    assert_eq!(
        written(&log),
        vec![
            entry("a", 70),
            entry("b", 70),
            entry("a", 80),
            entry("b", 80)
        ]
    );
    let outcomes = seen.lock().unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.failed == ["a".to_owned()]));
}

#[test]
fn panel_name_uses_the_model_segment() {
    assert_eq!(
        panel_name(r"DISPLAY\AUS27FD\5&efad&0&UID4353_0"),
        "Built-in display (AUS27FD)"
    );
    assert_eq!(panel_name("no-separators"), "Built-in display");
    assert_eq!(panel_name(r"DISPLAY\"), "Built-in display");
    assert_eq!(panel_name(""), "Built-in display");
}

#[test]
fn only_the_newest_run_token_is_current() {
    let tokens = RunTokens::default();
    let first = tokens.begin();
    assert!(tokens.is_current(first));
    let second = tokens.begin();
    assert_ne!(first, second);
    assert!(
        !tokens.is_current(first),
        "a zombie run is no longer current"
    );
    assert!(tokens.is_current(second));
    assert_eq!(tokens.latest(), second);
}

#[test]
fn panel_event_opens_the_window_then_sends() {
    use super::panel_event::on_panel_event;
    use crate::flyout::SuppressionWindow;
    use std::time::Instant;

    let t0 = Instant::now();
    let window = SuppressionWindow::new(t0);
    let mut open_when_sent = None;
    on_panel_event(Some(&window), t0, || {
        open_when_sent = Some(window.is_open(t0));
    });
    assert_eq!(open_when_sent, Some(true));
}

#[test]
fn no_window_still_sends() {
    use super::panel_event::on_panel_event;
    use std::time::Instant;

    let mut sent = false;
    on_panel_event(None, Instant::now(), || sent = true);
    assert!(sent);
}
