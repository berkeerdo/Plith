//! Composes the engine slice: process setup, monitor, shell window, render thread, the demo or
//! idle-check driver, the message loop and a clean shutdown.

mod backstop;
mod feed;
mod idle_facts;
mod idle_wiring;
mod input;
mod osd_slot;
mod saver;
mod sheet_side;
mod state;
mod weather_link;
mod wire;

#[cfg(test)]
mod tests;

use crate::instance;
use crate::live;
use crate::log::{self, LogFile};
use crate::services::fixture::{FixtureServices, TICK, Ticker};
use crate::services::start::StartBoard;
use crate::services::{LevelSource, RealServices, Services};
use crate::settings_file;
use crate::{demo, idle};
use feed::Feed;
use osd_slot::{OsdSlot, RealOsd};
use plith_core::demo::DemoStep;
use plith_core::launch::Launch;
use plith_core::layout::SurfaceLayout;
use plith_core::scene::{Scene, SceneMailbox, ShapeCell};
use plith_core::settings::Settings;
use plith_render::{RenderConfig, RenderHandle, spawn};
use plith_shell::{
    MonitorInfo, QuitHandle, Shell, UiWaker, init_process, init_ui_thread, is_remote_session,
    primary_monitor, run_message_loop, uninit_ui_thread, user_busy,
};
use saver::Saver;
use state::App;
use state::OsdControl;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
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
    // Probing runs (demo, idle check) skip the busy-user check below. They and fixture dev runs
    // also skip the guard, so measurements and dev runs work beside a running notch. The guard
    // lives until this function returns.
    let probing = launch.demo.is_some() || launch.idle_check_secs.is_some();
    let _instance = if probing || launch.fixture.is_some() {
        None
    } else {
        match instance::acquire(instance::INSTANCE_NAME) {
            Ok(Some(guard)) => Some(guard),
            Ok(None) => {
                log.line("single instance: another plith runs");
                return 0;
            }
            Err(error) => {
                log.line(&format!("single instance check failed: {error}"));
                None
            }
        }
    };
    if let Err(error) = init_process() {
        log.line(&format!("dpi awareness not set: {error}"));
    }
    let ole = init_ui_thread();
    if let Err(error) = &ole {
        log.line(&format!("ole not initialized: {error}"));
    }
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
    let shapes = Arc::new(ShapeCell::default());
    let app = Rc::new(RefCell::new(App::new(
        Arc::clone(&mailbox),
        Arc::clone(&shapes),
        Arc::clone(log),
        Arc::clone(&closing),
    )));
    let handler = {
        let app = Rc::clone(&app);
        Box::new(move |event| app.borrow_mut().on_event(event))
    };
    let on_panic = wire::on_panic(log, &fatal, &app);
    let shell = match Shell::create(layout, shapes, handler, on_panic) {
        Ok(shell) => shell,
        Err(error) => {
            log.line(&format!("cannot create the window: {error}"));
            return 1;
        }
    };
    shell.show();
    let quit = shell.quit_handle();
    let Attached {
        levels,
        content,
        board,
        settings,
    } = attach_feed(launch, &shell, log, &app);
    start_sides(launch, log, &shell, &app, &settings, monitor);
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
    let steps = attach_render_and_demo(&app, &render);
    let stop = Arc::new(AtomicBool::new(false));
    let fired = Arc::new(AtomicBool::new(false));
    let helpers = Helpers {
        launch,
        render: &render,
        log,
        stop: &stop,
        fired: &fired,
        quit,
        waker: shell.ui_waker(),
        steps,
        content,
        board,
    };
    let (loop_code, idle_ok) = drive(&helpers);
    app.borrow_mut().finish_settings();
    // The `Closing` handler normally joined the renderer already (this call is then a no-op).
    // Otherwise the window may already be gone: join before the `Shell` drops, and treat a
    // failure from here on as part of the close.
    closing.store(true, Ordering::Release);
    backstop::close_render(log, &render.stop_handle());
    render.shutdown();
    app.borrow_mut().stop_feed();
    // Suppression off and flyouts shown first, then the link sends `Bye`.
    app.borrow_mut().stop_osd();
    log.line(&wire::hit_line(&shell));
    wire::exit_code(
        loop_code,
        fatal.load(Ordering::Acquire),
        fired.load(Ordering::Acquire),
        idle_ok,
    )
}

/// What attaching the feed hands back to the drivers.
struct Attached {
    /// The level reader for the render thread.
    levels: LevelSource,
    /// The fixture's content variant requests; `None` with real services.
    content: Option<Sender<u8>>,
    /// The finished starts of the real services; `None` with the fixture.
    board: Option<Arc<StartBoard>>,
    /// The settings of the run, read once.
    settings: Settings,
}

/// The fixture services; with an idle check they also tick every [`TICK`], as a service
/// publishing at rest.
fn fixture(
    launch: &Launch,
    kind: plith_core::launch::Fixture,
    shell: &Shell,
    log: &LogFile,
    content: Receiver<u8>,
) -> FixtureServices {
    log.line(&format!("services: fixture {kind:?}"));
    let waker = shell.ui_waker();
    let services = FixtureServices::new(kind)
        .with_waker(Box::new(move || waker.wake()))
        .with_content(content);
    if launch.idle_check_secs.is_none() {
        return services;
    }
    let (tx, rx) = mpsc::channel();
    let waker = shell.ui_waker();
    match Ticker::spawn(TICK, tx, Box::new(move || waker.wake())) {
        Ok(ticker) => {
            log.line("services: fixture ticks every 250 ms");
            services.with_ticks(rx).with_ticker(ticker)
        }
        Err(error) => {
            log.line(&format!("services: no fixture ticker: {error}"));
            services
        }
    }
}

/// Starts the services (or the fixture), attaches the feed that turns them into pages and
/// returns the level reader for the render thread. Never blocks on a service start.
fn attach_feed(
    launch: &Launch,
    shell: &Shell,
    log: &Arc<LogFile>,
    app: &Rc<RefCell<App>>,
) -> Attached {
    let (mut content, mut board) = (None, None);
    let services: Box<dyn Services> = if let Some(kind) = launch.fixture {
        let (tx, rx) = mpsc::channel();
        content = Some(tx);
        Box::new(fixture(launch, kind, shell, log, rx))
    } else {
        let started = Arc::new(StartBoard::default());
        board = Some(Arc::clone(&started));
        Box::new(RealServices::start(
            &shell.ui_waker(),
            Arc::clone(log),
            &started,
        ))
    };
    let levels = services.level_source();
    let mut app = app.borrow_mut();
    // The language is resolved once, here, for every run mode (normal, demo, fixture, idle check).
    let settings = idle_wiring::settings_for(launch, log);
    let lang = idle_wiring::language_of(&settings);
    let mut feed = Feed::new(
        services,
        Box::new(shell.timer_handle()),
        Box::new(log::now),
        lang,
        &app.mailbox,
    );
    let weather = idle_wiring::attach(
        launch,
        log,
        shell.ui_waker(),
        &mut feed,
        &app.mailbox,
        &settings,
    );
    app.attach_feed(feed);
    app.attach_weather(weather, idle_wiring::is_normal(launch));
    app.set_settings(settings.clone());
    Attached {
        levels,
        content,
        board,
        settings,
    }
}

/// Gives the app its OSD slot and starts the side when `settings` want it, in a normal run with
/// real services only: demo, idle-check and fixture runs stay measurement runs without a helper
/// process. The side's sink reaches the feed through the app.
fn start_osd(
    launch: &Launch,
    log: &Arc<LogFile>,
    shell: &Shell,
    app: &Rc<RefCell<App>>,
    settings: &Settings,
) {
    if !idle_wiring::is_normal(launch) {
        return;
    }
    let waker = shell.ui_waker();
    let slot = OsdSlot::new(
        RealOsd {
            log: Arc::clone(log),
        },
        Arc::new(move || waker.wake()),
    );
    app.borrow_mut()
        .attach_osd(OsdControl::new(slot, launch.flyout), settings);
}

/// Tells the app how to stop the render thread and hands it the demo's scene steps; returns
/// the sender the demo thread posts them through.
fn attach_render_and_demo(app: &Rc<RefCell<App>>, render: &RenderHandle) -> Sender<DemoStep> {
    let mut app = app.borrow_mut();
    app.attach_render(render.stop_handle());
    let (steps, demo_steps) = mpsc::channel();
    app.attach_demo(demo_steps);
    steps
}

/// Starts the OSD side and attaches the settings side.
fn start_sides(
    launch: &Launch,
    log: &Arc<LogFile>,
    shell: &Shell,
    app: &Rc<RefCell<App>>,
    settings: &Settings,
    monitor: &MonitorInfo,
) {
    start_osd(launch, log, shell, app, settings);
    attach_settings_side(launch, log, shell, app, monitor);
}

/// Lets the quick page open the settings window, and in a normal run saves every change: probing
/// runs never write the settings file.
fn attach_settings_side(
    launch: &Launch,
    log: &Arc<LogFile>,
    shell: &Shell,
    app: &Rc<RefCell<App>>,
    monitor: &MonitorInfo,
) {
    let mut app = app.borrow_mut();
    let waker = shell.ui_waker();
    app.attach_sheet(monitor.handle, Arc::new(move || waker.wake()));
    if !idle_wiring::is_normal(launch) {
        return;
    }
    match settings_file::default_path() {
        Ok(path) => app.attach_saver(Saver::spawn(path, shell.ui_waker(), Arc::clone(log))),
        Err(error) => log.line(&format!(
            "settings: no folder, changes are not saved: {error}"
        )),
    }
}

/// Everything the helper threads and the message loop share.
struct Helpers<'a> {
    launch: &'a Launch,
    render: &'a RenderHandle,
    log: &'a Arc<LogFile>,
    stop: &'a Arc<AtomicBool>,
    fired: &'a Arc<AtomicBool>,
    quit: QuitHandle,
    waker: UiWaker,
    steps: Sender<DemoStep>,
    content: Option<Sender<u8>>,
    board: Option<Arc<StartBoard>>,
}

/// Starts the demo, idle and watchdog threads, runs the message loop, then stops and joins them.
/// Returns the loop's exit code and the idle verdict, if an idle check ran.
fn drive(h: &Helpers) -> (i32, Option<bool>) {
    let (launch, quit) = (h.launch, h.quit);
    thread::scope(|scope| {
        let idle_thread = launch.idle_check_secs.map(|secs| {
            scope.spawn(move || {
                let look = idle_wiring::settings_for(launch, h.log).look;
                let ok = idle::run(look, secs, h.render, h.log, h.stop, h.board.as_deref());
                quit.quit();
                ok
            })
        });
        let demo_thread = launch.demo.clone().map(|script| {
            let quit_after = launch.exit_after_demo.then_some(quit);
            demo::spawn(
                script,
                Arc::clone(h.stop),
                quit_after,
                demo::Links {
                    waker: h.waker.clone(),
                    steps: h.steps.clone(),
                    content: h.content.clone(),
                    log: Arc::clone(h.log),
                },
            )
        });
        let watchdog = (launch.demo.is_some() || launch.idle_check_secs.is_some()).then(|| {
            let secs = wire::watchdog_secs(launch.idle_check_secs, h.board.is_some());
            wire::spawn_watchdog(secs, h.stop, quit, h.log, h.fired)
        });
        let loop_code = run_message_loop();
        h.stop.store(true, Ordering::Release);
        for handle in [demo_thread, watchdog].into_iter().flatten() {
            let _ = handle.join();
        }
        let idle_ok = idle_thread.map(|handle| handle.join().unwrap_or(false));
        (loop_code, idle_ok)
    })
}
