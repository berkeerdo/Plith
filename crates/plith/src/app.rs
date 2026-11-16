//! Composes the engine slice: process setup, monitor, shell window, render thread, the demo or
//! idle-check driver, the message loop and a clean shutdown.

mod backstop;
mod feed;
mod state;
mod wire;

use crate::live;
use crate::log::{self, LogFile};
use crate::services::fixture::FixtureServices;
use crate::services::{LevelSource, RealServices, Services};
use crate::{demo, idle};
use feed::Feed;
use plith_core::launch::Launch;
use plith_core::layout::SurfaceLayout;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_render::{RenderConfig, RenderHandle, spawn};
use plith_shell::{
    MonitorInfo, QuitHandle, Shell, UiWaker, init_process, init_ui_thread, is_remote_session,
    primary_monitor, run_message_loop, uninit_ui_thread, user_busy,
};
use state::App;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    let ole = init_ui_thread();
    if let Err(error) = &ole {
        log.line(&format!("ole not initialized: {error}"));
    }
    let probing = launch.demo.is_some() || launch.idle_check_secs.is_some();
    if probing && let Some(state) = user_busy() {
        log.line(&format!("skipped: user busy ({state})"));
        if ole.is_ok() {
            uninit_ui_thread();
        }
        return wire::EXIT_USER_BUSY;
    }
    let monitor = primary_monitor();
    let layout = SurfaceLayout::new(monitor.rect, monitor.work_top_px, monitor.scale);
    log.line(&wire::start_line(launch, &monitor, &layout));
    let code = run_shell(launch, log, &monitor, layout);
    if ole.is_ok() {
        uninit_ui_thread();
    }
    code
}

fn run_shell(
    launch: &Launch,
    log: &Arc<LogFile>,
    monitor: &MonitorInfo,
    layout: SurfaceLayout,
) -> i32 {
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    let closing = Arc::new(AtomicBool::new(false));
    let fatal = Arc::new(AtomicBool::new(false));
    let app = Rc::new(RefCell::new(App::new(
        Arc::clone(&mailbox),
        Arc::clone(log),
        Arc::clone(&closing),
    )));
    let handler = {
        let app = Rc::clone(&app);
        Box::new(move |event| app.borrow_mut().on_event(event))
    };
    let on_panic = wire::on_panic(log, &fatal, &app);
    let shell = match Shell::create(layout, Arc::new(ShapeCell::default()), handler, on_panic) {
        Ok(shell) => shell,
        Err(error) => {
            log.line(&format!("cannot create the window: {error}"));
            return 1;
        }
    };
    shell.show();
    let quit = shell.quit_handle();
    let levels = attach_feed(launch, &shell, log, &app);
    let config = RenderConfig {
        hwnd: shell.hwnd(),
        monitor: monitor.handle,
        layout,
        clock: launch.clock,
        remote: is_remote_session(),
        hz: monitor.hz,
        levers: launch.levers,
        tune: launch.tune,
    };
    let sample = live::sampler(levels);
    let sinks = wire::sinks(shell.shape_poster(), log, quit, &fatal, &closing, sample);
    let render = match spawn(config, Arc::clone(&mailbox), sinks) {
        Ok(render) => render,
        Err(error) => {
            log.line(&format!("render failed to start: {error}"));
            return 1;
        }
    };
    app.borrow_mut().attach_render(render.stop_handle());
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
        waker: shell.ui_waker(),
    };
    let (loop_code, idle_ok) = drive(&helpers);
    // The `Closing` handler normally joined the renderer already (this call is then a no-op).
    // Otherwise the window may already be gone: join before the `Shell` drops, and treat a
    // failure from here on as part of the close.
    closing.store(true, Ordering::Release);
    backstop::close_render(log, &render.stop_handle());
    render.shutdown();
    app.borrow_mut().stop_feed();
    log.line(&wire::hit_line(&shell));
    wire::exit_code(
        loop_code,
        fatal.load(Ordering::Acquire),
        fired.load(Ordering::Acquire),
        idle_ok,
    )
}

/// Starts the services (or the fixture), attaches the feed that turns them into pages and
/// returns the level reader for the render thread. Never blocks on a service start.
fn attach_feed(
    launch: &Launch,
    shell: &Shell,
    log: &Arc<LogFile>,
    app: &Rc<RefCell<App>>,
) -> LevelSource {
    let services: Box<dyn Services> = match launch.fixture {
        Some(kind) => {
            log.line(&format!("services: fixture {kind:?}"));
            let waker = shell.ui_waker();
            Box::new(FixtureServices::new(kind).with_waker(Box::new(move || waker.wake())))
        }
        None => Box::new(RealServices::start(&shell.ui_waker(), Arc::clone(log))),
    };
    let levels = services.level_source();
    let mut app = app.borrow_mut();
    let feed = Feed::new(
        services,
        Box::new(shell.timer_handle()),
        Box::new(log::now),
        &app.mailbox,
    );
    app.attach_feed(feed);
    levels
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
    waker: UiWaker,
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
                h.waker.clone(),
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
