//! The demo driver: plays a `DemoScript` by publishing scenes. It never synthesises input.

use crate::log::LogFile;
use plith_core::demo::{DemoScript, DemoStep};
use plith_core::scene::{Scene, SceneMailbox};
use plith_shell::{QuitHandle, UiWaker};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// How long a sleep may go on before it looks at the stop flag again.
const CHUNK_MS: u64 = 50;

/// Time for the last motion to finish and report before the window closes.
const LAST_REPORT_MS: u64 = 1500;

/// The scene after `step`; a wait changes nothing and the suppression is always kept.
#[must_use]
pub fn apply(step: DemoStep, scene: Scene) -> Scene {
    match step {
        DemoStep::Expand => Scene {
            expanded: true,
            ..scene
        },
        DemoStep::Collapse => Scene {
            expanded: false,
            ..scene
        },
        DemoStep::Page(page) => Scene {
            expanded: true,
            page,
            ..scene
        },
        DemoStep::Wait(_) | DemoStep::Content(_) => scene,
    }
}

/// Sleeps `ms` in 50 ms chunks; returns false when `stop` was set before the time was up.
#[must_use]
pub fn sleep_stoppable(ms: u64, stop: &AtomicBool) -> bool {
    let mut left = ms;
    loop {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        if left == 0 {
            return true;
        }
        let chunk = left.min(CHUNK_MS);
        thread::sleep(Duration::from_millis(chunk));
        left -= chunk;
    }
}

/// What a `content` step does: send the variant to the fixture services, or, without a fixture,
/// log once that content steps are ignored. Never touches the real services.
pub fn content_requester(services: Option<Sender<u8>>, log: &dyn Fn(&str)) -> impl Fn(u8) {
    let told = Cell::new(false);
    move |variant| match &services {
        Some(tx) => {
            let _ = tx.send(variant);
        }
        None if !told.replace(true) => {
            log("demo: content steps are ignored without --fixture");
        }
        None => {}
    }
}

/// Plays the script into the mailbox; returns false when stopped before the end. Each step reads
/// the latest scene, so a click or a suppression change in between is respected. `wake` runs
/// after every scene step, so the UI thread sees the new scene. A `content` step hands its variant
/// to `content` and then wakes the UI thread, which drains the services.
#[must_use]
pub fn play(
    script: &DemoScript,
    mailbox: &SceneMailbox,
    stop: &AtomicBool,
    wake: &dyn Fn(),
    content: &dyn Fn(u8),
) -> bool {
    if script.steps.is_empty() {
        return true;
    }
    for _ in 0..script.repeat {
        for step in &script.steps {
            if stop.load(Ordering::Acquire) {
                return false;
            }
            if let DemoStep::Wait(ms) = step {
                if !sleep_stoppable(*ms, stop) {
                    return false;
                }
            } else if let DemoStep::Content(variant) = step {
                content(*variant);
                wake();
            } else {
                let _ = mailbox.update(|scene| apply(*step, scene));
                wake();
            }
        }
    }
    true
}

/// What the demo thread reaches outside the mailbox.
pub struct Links {
    /// Wakes the UI thread after a scene or content step.
    pub waker: UiWaker,
    /// The fixture services' content channel; `None` without `--fixture`.
    pub content: Option<Sender<u8>>,
    /// The log, for the one line about ignored content steps.
    pub log: Arc<LogFile>,
}

/// Plays `script` on its own thread. When it ends and `quit_after` is set, waits for the last
/// motion's report and then closes the window. `content` reaches the fixture services, if any.
pub fn spawn(
    script: DemoScript,
    mailbox: Arc<SceneMailbox>,
    stop: Arc<AtomicBool>,
    quit_after: Option<QuitHandle>,
    links: Links,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let log_line = |line: &str| links.log.line(line);
        let request = content_requester(links.content, &log_line);
        let finished = play(&script, &mailbox, &stop, &|| links.waker.wake(), &request);
        if let (true, Some(quit)) = (finished, quit_after)
            && sleep_stoppable(LAST_REPORT_MS, &stop)
        {
            quit.quit();
        }
    })
}

#[cfg(test)]
mod tests;
