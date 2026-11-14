//! Composes the engine slice: process setup, monitor, shell window, render thread, the demo or
//! idle-check driver, the message loop and a clean shutdown.

mod wire;

use crate::log::LogFile;
use crate::{demo, idle};
use plith_core::launch::Launch;
use plith_core::layout::SurfaceLayout;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_render::{RenderConfig, RenderHandle, spawn};
use plith_shell::{
    MonitorInfo, QuitHandle, Shell, init_process, is_remote_session, primary_monitor,
    run_message_loop, user_busy,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;

/// Runs the app until its window closes and returns the process exit code.
#[must_use]
pub fn run(launch: &Launch) -> i32 {
    let log = match LogFile::open() {
        Ok(log) => Arc::new(log),
        Err(error) => {
            eprintln!("cannot open the log file: {error}");
            return 1;
        }
    };
    let code = run_logged(launch, &log);
    log.line(&format!("exit {code}"));
    code
}

fn run_logged(launch: &Launch, log: &Arc<LogFile>) -> i32 {
    if let Err(error) = init_process() {
        log.line(&format!("dpi awareness not set: {error}"));
    }
    let probing = launch.demo.is_some() || launch.idle_check_secs.is_some();
    if probing && let Some(state) = user_busy() {
        log.line(&format!("skipped: user busy ({state})"));
        return wire::EXIT_USER_BUSY;
    }
    let monitor = primary_monitor();
    let layout = SurfaceLayout::new(monitor.rect, monitor.work_top_px, monitor.scale);
    log.line(&wire::start_line(launch, &monitor, &layout));
    run_shell(launch, log, &monitor, layout)
}

fn run_shell(
    launch: &Launch,
    log: &Arc<LogFile>,
    monitor: &MonitorInfo,
    layout: SurfaceLayout,
) -> i32 {
    let mailbox = Arc::new(SceneMailbox::new(Scene::default()));
    let closing = Arc::new(AtomicBool::new(false));
    let stopper = Arc::new(OnceLock::new());
    let handler = wire::event_handler(&mailbox, log, &closing, &stopper);
    let shell = match Shell::create(layout, launch.hit, Arc::new(ShapeCell::default()), handler) {
        Ok(shell) => shell,
        Err(error) => {
            log.line(&format!("cannot create the window: {error}"));
            return 1;
        }
    };
    shell.show();
    let quit = shell.quit_handle();
    let fatal = Arc::new(AtomicBool::new(false));
    let config = RenderConfig {
        hwnd: shell.hwnd(),
        monitor: monitor.handle,
        layout,
        clock: launch.clock,
        remote: is_remote_session(),
        hz: monitor.hz,
    };
    let sinks = wire::sinks(shell.shape_poster(), log, quit, &fatal, &closing);
    let render = match spawn(config, Arc::clone(&mailbox), sinks) {
        Ok(render) => render,
        Err(error) => {
            log.line(&format!("render failed to start: {error}"));
            return 1;
        }
    };
    let _ = stopper.set(render.stop_handle());
    let stop = Arc::new(AtomicBool::new(false));
    let fired = Arc::new(AtomicBool::new(false));
    let helpers = Helpers {
        launch,
        mailbox: &mailbox,
        render: &render,
        log,
        stop: &stop,
        fired: &fired,
        quit,
    };
    let (loop_code, idle_ok) = drive(&helpers);
    // The window may already be destroyed: stop and join the renderer before the `Shell` drops,
    // and treat a failure from here on as part of the close.
    closing.store(true, Ordering::Release);
    render.shutdown();
    log.line(&wire::hit_line(&shell));
    wire::exit_code(
        loop_code,
        fatal.load(Ordering::Acquire),
        fired.load(Ordering::Acquire),
        idle_ok,
    )
}

/// Everything the helper threads and the message loop share.
struct Helpers<'a> {
    launch: &'a Launch,
    mailbox: &'a Arc<SceneMailbox>,
    render: &'a RenderHandle,
    log: &'a Arc<LogFile>,
    stop: &'a Arc<AtomicBool>,
    fired: &'a Arc<AtomicBool>,
    quit: QuitHandle,
}

/// Starts the demo, idle and watchdog threads, runs the message loop, then stops and joins them.
/// Returns the loop's exit code and the idle verdict, if an idle check ran.
fn drive(h: &Helpers) -> (i32, Option<bool>) {
    let (launch, quit) = (h.launch, h.quit);
    thread::scope(|scope| {
        let idle_thread = launch.idle_check_secs.map(|secs| {
            scope.spawn(move || {
                let ok = idle::run(secs, h.render, h.log, h.stop);
                quit.quit();
                ok
            })
        });
        let demo_thread = launch.demo.clone().map(|script| {
            let quit_after = launch.exit_after_demo.then_some(quit);
            demo::spawn(
                script,
                Arc::clone(h.mailbox),
                Arc::clone(h.stop),
                quit_after,
            )
        });
        let watchdog = (launch.demo.is_some() || launch.idle_check_secs.is_some())
            .then(|| wire::spawn_watchdog(h.stop, quit, h.log, h.fired));
        let loop_code = run_message_loop();
        h.stop.store(true, Ordering::Release);
        for handle in [demo_thread, watchdog].into_iter().flatten() {
            let _ = handle.join();
        }
        let idle_ok = idle_thread.map(|handle| handle.join().unwrap_or(false));
        (loop_code, idle_ok)
    })
}
