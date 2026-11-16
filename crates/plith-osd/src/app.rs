//! The OSD helper: binds the pipe, creates the band window and runs three threads.
//!
//! - UI thread (this one): owns the window, pumps its messages, draws nothing.
//! - Server thread: `OsdServer::serve`; every decoded message goes to the render thread over a
//!   bounded channel with a non-blocking send (a full channel drops the message, logged once).
//! - Render thread ([`crate::render`]): owns every graphics object and the model.
//!
//! The render thread ends the app: it posts [`WM_APP_EXIT`] when the model says exit.

use crate::band::{self, BandWindow, Creation};
use crate::log::Log;
use crate::paint::{HEIGHT, WIDTH};
use crate::render::{self, Event, Job, WM_APP_EXIT};
use crate::watch;
use plith_core::layout::SurfaceLayout;
use plith_ipc::{BindError, DecodeError, Flow, Message, Meter, OsdServer, ServerEvent};
use std::mem::{Discriminant, discriminant};
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::Instant;
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
            log.line(&format!("osd: {pipe} is already served, exiting"));
            return Ok(());
        }
        Err(error) => return Err(format!("bind {pipe}: {error}")),
    };
    let (monitor, scale) = band::primary_monitor()?;
    let surface = band::top_centre(monitor, scale, f64::from(WIDTH), f64::from(HEIGHT));
    let window = create_window(surface, log)?;
    // The window stays hidden until the first frame is drawn.
    band::set_visible(window.hwnd, false);
    let hz = band::refresh_hz();
    log.line(&format!(
        "osd start: {} band={} uiaccess={} rect={},{} {}x{} scale={scale} hz={hz} pipe={pipe}",
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
        layout: SurfaceLayout { surface, scale },
        hz,
        origin: Instant::now(),
        events: inbox,
        log: Arc::clone(log),
    };
    let renderer = spawn("plith-osd-render", move || render::run(&job))?;
    let stopper = server.stopper();
    let server_log = Arc::clone(log);
    let server_events = events.clone();
    let server = spawn("plith-osd-server", move || {
        serve(&server, &server_events, &server_log)
    })?;
    pump();
    stopper.stop();
    // Blocks only while the render thread still runs and drains; fails at once once it ended.
    let _ = events.send(Event::Quit);
    if let Err(error) = join(server) {
        log.line(&format!("osd server: {error}"));
    }
    let result = join(renderer);
    drop(window);
    result
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

/// The UIAccess band when Windows allows it (signed, installed), else a plain topmost window.
fn create_window(surface: plith_core::layout::RectPx, log: &Log) -> Result<BandWindow, String> {
    band::create_band_window(surface, Creation::InBand).or_else(|error| {
        log.line(&format!(
            "osd: inband refused ({error}), falling back to plain"
        ));
        band::create_band_window(surface, Creation::Plain)
    })
}

/// Pumps this thread's messages until the render thread posts `WM_APP_EXIT` or `WM_QUIT`.
fn pump() {
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a live local; a null window takes every message of this thread.
        let got = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        if got.0 <= 0 || msg.message == WM_APP_EXIT {
            return;
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
    }
}

/// What the server callback remembers between messages.
#[derive(Default)]
struct Served {
    /// Decode error kinds already logged.
    errors: Vec<Discriminant<DecodeError>>,
    /// Whether a full channel was already logged.
    dropped: bool,
    /// The pid whose watcher runs.
    watched: Option<u32>,
}

impl Served {
    fn on(
        &mut self,
        got: Result<Message, DecodeError>,
        events: &SyncSender<Event>,
        log: &Log,
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
        if let Message::Hello { client_pid } = message
            && self.watched != Some(client_pid)
        {
            self.watched = Some(client_pid);
            if let Err(error) = watch::client(client_pid, events.clone()) {
                log.line(&format!("osd: {error}"));
            }
        }
        match events.try_send(Event::Message(message)) {
            Ok(()) => Flow::Continue,
            Err(TrySendError::Full(_)) => {
                if !std::mem::replace(&mut self.dropped, true) {
                    log.line("osd: render channel full, dropping messages (logged once)");
                }
                Flow::Continue
            }
            Err(TrySendError::Disconnected(_)) => Flow::Stop,
        }
    }
}

/// The server thread: serves clients until stopped. A server failure asks the render thread
/// to quit, which ends the app.
fn serve(server: &OsdServer, events: &SyncSender<Event>, log: &Log) -> Result<(), String> {
    let mut state = Served::default();
    let result = server.serve(|event| match event {
        ServerEvent::Message(got) => state.on(got, events, log),
        ServerEvent::Connected | ServerEvent::Disconnected => Flow::Continue,
    });
    if result.is_err() {
        let _ = events.try_send(Event::Quit);
    }
    result
}

#[cfg(test)]
mod tests;
