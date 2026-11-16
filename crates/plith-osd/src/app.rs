//! The OSD helper: binds the pipe, creates the band window and runs three threads.
//!
//! - UI thread (this one): owns the window, pumps its messages, draws nothing. After a display
//!   or DPI change it re-places the window at the primary monitor's top centre and sends the
//!   render thread [`Event::Relayout`].
//! - Server thread: `OsdServer::serve`; every decoded message goes to the render thread over a
//!   bounded channel with a blocking send. The render thread drains it quickly, so a final
//!   `Level`, `Hide` or `Bye` is never the one dropped.
//! - Render thread ([`crate::render`]): owns every graphics object and the model.
//!
//! The render thread ends the app: it posts [`WM_APP_EXIT`] when the model says exit.

use crate::band::{self, BandWindow, Creation};
use crate::log::Log;
use crate::paint::{HEIGHT, WIDTH};
use crate::render::{self, Event, Job, WM_APP_EXIT};
use crate::watch;
use plith_core::layout::{RectPx, SurfaceLayout};
use plith_ipc::{BindError, DecodeError, Flow, Message, Meter, OsdServer, ServerEvent};
use std::mem::{Discriminant, discriminant};
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread;
use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};

/// Room for messages between two render wakes.
const CHANNEL: usize = 64;

/// Runs the helper on `pipe` until the client says `Bye` or goes away. Another running helper
/// of this user is not an error: this one logs it and returns at once.
///
/// # Errors
/// Returns why the helper could not run: the pipe is held by someone else, the window or a
/// thread could not be created, or the render thread failed.
pub fn run(pipe: &str, log: &Arc<Log>) -> Result<(), String> {
    let server = match OsdServer::bind(pipe) {
        Ok(server) => server,
        Err(BindError::AlreadyRunning) => {
            log.line(&format!("osd: {pipe:?} is already served, exiting"));
            return Ok(());
        }
        Err(error) => return Err(format!("bind {pipe:?}: {error}")),
    };
    let (monitor, scale) = band::primary_monitor()?;
    let layout = placement(monitor, scale);
    let surface = layout.surface;
    let window = create_window(surface, log)?;
    // The window stays hidden until the first frame is drawn.
    band::set_visible(window.hwnd, false);
    let hz = band::refresh_hz();
    log.line(&format!(
        "osd start: {} band={} uiaccess={} rect={},{} {}x{} scale={scale} hz={hz} pipe={pipe:?}",
        window.creation.as_str(),
        window
            .band
            .map_or_else(|| String::from("unknown"), |b| b.to_string()),
        band::token_ui_access().map_or_else(|e| e, |flag| flag.to_string()),
        surface.x,
        surface.y,
        surface.width,
        surface.height,
    ));
    let (events, inbox) = sync_channel(CHANNEL);
    let job = Job {
        hwnd: window.hwnd.0 as usize,
        layout,
        hz,
        origin: Instant::now(),
        events: inbox,
        log: Arc::clone(log),
    };
    let renderer = spawn("plith-osd-render", move || render::run(&job))?;
    let stopper = server.stopper();
    let server_log = Arc::clone(log);
    let server_events = events.clone();
    let server = match spawn("plith-osd-server", move || {
        serve(&server, &server_events, &server_log)
    }) {
        Ok(handle) => handle,
        Err(error) => {
            let _ = events.send(Event::Quit);
            if let Err(render) = join(renderer) {
                log.line(&format!("osd render: {render}"));
            }
            return Err(error);
        }
    };
    pump(|| relayout(window.hwnd, &events, log));
    stopper.stop();
    // Blocks only while the render thread still runs and drains; fails at once once it ended.
    let _ = events.send(Event::Quit);
    let server_end = join(server);
    if let Err(error) = &server_end {
        log.line(&format!("osd server: {error}"));
    }
    let result = join(renderer);
    drop(window);
    // A render failure wins; else a failed or panicked server thread fails the run.
    result.and(server_end.map_err(|error| format!("server thread: {error}")))
}

fn spawn(
    name: &str,
    body: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<thread::JoinHandle<Result<(), String>>, String> {
    thread::Builder::new()
        .name(name.to_string())
        .spawn(body)
        .map_err(|e| format!("spawn {name}: {e}"))
}

fn join(handle: thread::JoinHandle<Result<(), String>>) -> Result<(), String> {
    handle
        .join()
        .unwrap_or_else(|_| Err(String::from("thread panicked")))
}

/// The OSD's place on a monitor: the seal's DIP size at `scale`, centred at the top of `monitor`.
#[must_use]
pub fn placement(monitor: RectPx, scale: f64) -> SurfaceLayout {
    SurfaceLayout {
        surface: band::top_centre(monitor, scale, f64::from(WIDTH), f64::from(HEIGHT)),
        scale,
    }
}

/// Re-reads the primary monitor after a display or DPI change, moves the window there and
/// asks the render thread to rebuild at the new size and scale. A failed read keeps the old
/// place (logged).
fn relayout(hwnd: HWND, events: &SyncSender<Event>, log: &Log) {
    if !band::take_relayout_request() {
        return;
    }
    let (monitor, scale) = match band::primary_monitor() {
        Ok(found) => found,
        Err(error) => {
            log.line(&format!("osd relayout: {error}, keeping the old place"));
            return;
        }
    };
    let layout = placement(monitor, scale);
    let r = layout.surface;
    log.line(&format!(
        "osd relayout: rect={},{} {}x{} scale={scale}",
        r.x, r.y, r.width, r.height
    ));
    if let Err(error) = band::move_window(hwnd, r) {
        log.line(&format!("osd relayout: {error}"));
    }
    // Blocking like every other event; the render thread never waits on this thread.
    let _ = events.send(Event::Relayout(layout));
}

/// The UIAccess band when Windows allows it (signed, installed), else a plain topmost window.
fn create_window(surface: RectPx, log: &Log) -> Result<BandWindow, String> {
    band::create_band_window(surface, Creation::InBand).or_else(|error| {
        log.line(&format!(
            "osd: inband refused ({error}), falling back to plain"
        ));
        band::create_band_window(surface, Creation::Plain)
    })
}

/// Pumps this thread's messages until the render thread posts `WM_APP_EXIT` or `WM_QUIT`;
/// calls `on_relayout` for every [`band::WM_APP_RELAYOUT`].
fn pump(mut on_relayout: impl FnMut()) {
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a live local; a null window takes every message of this thread.
        let got = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        if got.0 <= 0 || msg.message == WM_APP_EXIT {
            return;
        }
        if msg.message == band::WM_APP_RELAYOUT {
            on_relayout();
            continue;
        }
        // SAFETY: `msg` was filled by GetMessageW above.
        unsafe {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
}

/// The log text of one message.
#[must_use]
pub fn describe(message: &Message) -> String {
    match message {
        Message::Hello { client_pid } => format!("hello pid {client_pid}"),
        Message::Level {
            meter,
            percent,
            muted,
            label,
        } => {
            let meter = match meter {
                Meter::Volume => "volume",
                Meter::Brightness => "brightness",
            };
            format!("level {meter} {percent} muted={muted} label={label:?}")
        }
        Message::Hide => String::from("hide"),
        Message::Bye => String::from("bye"),
        Message::Ping => String::from("ping"),
    }
}

/// What the server callback remembers between messages.
#[derive(Default)]
struct Served {
    /// Decode error kinds already logged.
    errors: Vec<Discriminant<DecodeError>>,
    /// The watcher of the connected client (kernel pid), dropped when it disconnects.
    watcher: Option<watch::Watcher>,
}

impl Served {
    /// Handles one server event: logs it and forwards what the render thread needs.
    fn on(&mut self, event: ServerEvent, events: &SyncSender<Event>, log: &Arc<Log>) -> Flow {
        match event {
            ServerEvent::Connected { pid } => {
                log.line(&format!("osd pipe: client pid {pid} connected"));
                // The old watcher is cancelled and joined before the new one starts.
                self.watcher = None;
                match watch::client(pid, events.clone(), Arc::clone(log)) {
                    Ok(watcher) => self.watcher = Some(watcher),
                    Err(error) => log.line(&format!("osd: {error}")),
                }
                Flow::Continue
            }
            ServerEvent::Rejected(why) => {
                log.line(&format!("osd pipe: client rejected: {why}"));
                Flow::Continue
            }
            ServerEvent::Disconnected => {
                log.line("osd pipe: client disconnected");
                self.watcher = None;
                forward(events, Event::Disconnected)
            }
            ServerEvent::Message(got) => self.message(got, events, log),
        }
    }

    fn message(
        &mut self,
        got: Result<Message, DecodeError>,
        events: &SyncSender<Event>,
        log: &Arc<Log>,
    ) -> Flow {
        let message = match got {
            Ok(message) => message,
            Err(error) => {
                if !self.errors.contains(&discriminant(&error)) {
                    self.errors.push(discriminant(&error));
                    log.line(&format!(
                        "osd msg: decode error {error:?} (logged once per kind)"
                    ));
                }
                return Flow::Continue;
            }
        };
        log.line(&format!("osd msg: {}", describe(&message)));
        forward(events, Event::Message(message))
    }
}

/// Sends `event` to the render thread, waiting for room: the render thread drains quickly, and a
/// dropped `Hello`, `Level`, `Hide` or `Bye` would leave the OSD wrong. A closed channel
/// (the render thread ended) stops the server.
fn forward(events: &SyncSender<Event>, event: Event) -> Flow {
    match events.send(event) {
        Ok(()) => Flow::Continue,
        Err(_) => Flow::Stop,
    }
}

/// Sends [`Event::Quit`] when the server thread unwinds, so a panic in `serve` ends the
/// helper (the render thread quits, the UI thread follows) and the notch can relaunch it,
/// instead of leaving a High-integrity process with no pipe.
struct QuitOnPanic<'a>(&'a SyncSender<Event>);

impl Drop for QuitOnPanic<'_> {
    fn drop(&mut self) {
        if thread::panicking() {
            // Blocks only while the render thread runs; fails at once once it ended.
            let _ = self.0.send(Event::Quit);
        }
    }
}

/// The server thread: serves clients until stopped. A server failure asks the render thread
/// to quit (waiting for room in the channel), which ends the app.
fn serve(server: &OsdServer, events: &SyncSender<Event>, log: &Arc<Log>) -> Result<(), String> {
    let _quit_on_panic = QuitOnPanic(events);
    let mut state = Served::default();
    let result = server.serve(|event| state.on(event, events, log));
    if result.is_err() {
        let _ = events.send(Event::Quit);
    }
    result
}

#[cfg(test)]
mod tests;
