//! The demo driver: plays a `DemoScript` by publishing scenes. It never synthesises input.

use plith_core::demo::{DemoScript, DemoStep};
use plith_core::scene::{Scene, SceneMailbox};
use plith_shell::{QuitHandle, UiWaker};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
        DemoStep::Wait(_) => scene,
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

/// Plays the script into the mailbox; returns false when stopped before the end. Each step reads
/// the latest scene, so a click or a suppression change in between is respected. `wake` runs
/// after every scene step, so the UI thread sees the new scene.
#[must_use]
pub fn play(
    script: &DemoScript,
    mailbox: &SceneMailbox,
    stop: &AtomicBool,
    wake: &dyn Fn(),
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
            } else {
                let _ = mailbox.update(|scene| apply(*step, scene));
                wake();
            }
        }
    }
    true
}

/// Plays `script` on its own thread. When it ends and `quit_after` is set, waits for the last
/// motion's report and then closes the window.
pub fn spawn(
    script: DemoScript,
    mailbox: Arc<SceneMailbox>,
    stop: Arc<AtomicBool>,
    quit_after: Option<QuitHandle>,
    waker: UiWaker,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let finished = play(&script, &mailbox, &stop, &|| waker.wake());
        if let (true, Some(quit)) = (finished, quit_after)
            && sleep_stoppable(LAST_REPORT_MS, &stop)
        {
            quit.quit();
        }
    })
}

#[cfg(test)]
mod tests;
