//! The installer window at medium integrity: it shows the screens, starts the elevated worker
//! through the pump, and does the effects that belong to the user (the Run value, starting
//! Plith, the user's data).
//!
//! `Flow` (in `plith-core`) owns the screen state; this module only feeds it clicks, keys and
//! worker reports and carries out what it asks for.

mod art;
mod effects;
mod pump;

use crate::register::{Places, read_installed};
use crate::worker::install_root;
use plith_core::lang::{Lang, resolve};
use plith_core::settings::LangChoice;
use plith_core::setup::{
    Flow, Mode, Screen, SetupAction, SetupTarget, Situation, Version, setup_build, setup_hit,
    situation,
};
use plith_core::theme::Palette;
use plith_core::tray::run_value::is_ours;
use plith_host::autostart::RunKey;
use plith_host::locale::windows_language;
use plith_host::log::{LogFile, data_dir};
use plith_render::{SheetConfig, SheetRender};
use plith_shell::process::{init_ui_thread, uninit_ui_thread};
use plith_shell::setup_window::{SetupEvent, SetupWindow, find_open, quit, raise_other};
use plith_shell::window::run_message_loop;
use pump::{Inbox, Msg, send, take_all};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// The key codes the window acts on.
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;

/// Everything the window callback reaches. It lives behind an `Rc<RefCell<..>>` on the UI thread.
struct App {
    flow: Flow,
    lang: Lang,
    own: Version,
    window: Option<SetupWindow>,
    render: Option<SheetRender>,
    log: Option<LogFile>,
    inbox: Inbox,
    pump: Option<JoinHandle<()>>,
    /// The targets of the screen on show.
    targets: Vec<SetupTarget>,
    /// The screen last published, so an unchanged one is not painted again.
    shown: Option<Screen>,
    /// The install folder, when Windows names it.
    root: Option<PathBuf>,
    /// The process exit code: 1 once something broke.
    exit: i32,
}

/// Holds the single-instance mutex until dropped.
struct SingleInstance(HANDLE);

impl SingleInstance {
    /// `None` when another setup window of this user is open (it is raised instead).
    fn take() -> Option<Self> {
        // SAFETY: the name is a static NUL-terminated string; no attributes are passed.
        let handle = unsafe { CreateMutexW(None, false, w!("Local\\Plith.Setup")) }.ok()?;
        // SAFETY: reads this thread's last error, set by the call above.
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let guard = Self(handle);
        if existed {
            if let Some(other) = find_open() {
                raise_other(other);
            }
            return None;
        }
        Some(guard)
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// What is on the machine, from the Apps entry and the WPF app's `Plith.dll` (its `Plith.exe`
/// has the same name as ours, so the exe cannot tell them apart).
fn find_situation(own: Version, root: Option<&Path>) -> Situation {
    let entry = Places::production()
        .ok()
        .and_then(|places| read_installed(&places));
    let has_wpf = root.is_some_and(|root| root.join("Plith.dll").exists());
    situation(own, entry.as_deref(), has_wpf)
}

/// Runs the installer window for `mode` until it is closed and returns the exit code.
pub fn run(mode: &Mode) -> i32 {
    let Some(_single) = SingleInstance::take() else {
        return 0;
    };
    let ole = init_ui_thread().is_ok();
    let code = show(mode);
    if ole {
        uninit_ui_thread();
    }
    code
}

fn show(mode: &Mode) -> i32 {
    let log = data_dir()
        .ok()
        .and_then(|dir| LogFile::open_at(&dir.join("setup.log")).ok());
    let root = install_root().ok();
    let own = Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version::new(0, 0, 0));
    // The Run value as found: an update or repair keeps what the user chose.
    let run_value = RunKey::user_run()
        .read()
        .ok()
        .flatten()
        .map(|text| is_ours(&text));
    let flow = Flow::new(mode, find_situation(own, root.as_deref()), own, run_value);
    let inbox = Inbox::default();
    let app = Rc::new(RefCell::new(App {
        flow,
        lang: resolve(&LangChoice::Auto, &windows_language()),
        own,
        window: None,
        render: None,
        log,
        inbox: Arc::clone(&inbox),
        pump: None,
        targets: Vec::new(),
        shown: None,
        root,
        exit: 0,
    }));
    if let Err(text) = open(&app, &inbox) {
        app.borrow_mut().log(&text);
        app.borrow_mut().exit = 1;
    } else {
        let _ = run_message_loop();
    }
    let mut app = app.borrow_mut();
    app.finish();
    app.exit
}

/// Opens the window and its renderer and shows the first screen.
fn open(app: &Rc<RefCell<App>>, inbox: &Inbox) -> Result<(), String> {
    let on_event = {
        let app = Rc::clone(app);
        Box::new(move |event| app.borrow_mut().on_event(event))
    };
    let on_panic = {
        let app = Rc::clone(app);
        Box::new(move |text: String| {
            if let Ok(mut app) = app.try_borrow_mut() {
                app.log(&format!("panic: {text}"));
                app.exit = 1;
            }
            quit();
        })
    };
    let window = SetupWindow::open("Plith Setup", on_event, on_panic)
        .map_err(|error| format!("cannot open the window: {error}"))?;
    let (width_px, height_px, scale) = window.client_px();
    let hwnd = window.hwnd();
    let config = SheetConfig {
        hwnd,
        width_px,
        height_px,
        scale,
        palette: Palette::setup(),
        art: art::decode_all(scale)?,
    };
    let on_fatal = {
        let inbox = Arc::clone(inbox);
        Box::new(move |text: String| send(&inbox, hwnd, Msg::RenderFatal(text)))
    };
    let render = SheetRender::spawn(config, on_fatal)
        .map_err(|error| format!("cannot start the renderer: {error}"))?;
    let mut app = app.borrow_mut();
    app.window = Some(window);
    app.render = Some(render);
    app.refresh();
    Ok(())
}

impl App {
    fn log(&self, text: &str) {
        if let Some(log) = &self.log {
            log.line(text);
        }
    }

    /// Paints the screen of the flow when it differs from the one on show.
    fn refresh(&mut self) {
        let screen = self.flow.screen();
        if self.shown.as_ref() == Some(&screen) {
            return;
        }
        let (list, targets) = setup_build(&screen, self.own, self.lang);
        if let Some(window) = &self.window {
            window.set_targets(targets.clone());
        }
        self.targets = targets;
        if let Some(render) = &self.render {
            render.publish(Arc::new(list));
        }
        self.shown = Some(screen);
    }

    fn on_event(&mut self, event: SetupEvent) {
        let effects = match event {
            SetupEvent::Click(x, y) => setup_hit(&self.targets, x, y)
                .map(|action| self.flow.on_action(action))
                .unwrap_or_default(),
            SetupEvent::Key(VK_RETURN) => self
                .flow
                .primary_action()
                .map(|action| self.flow.on_action(action))
                .unwrap_or_default(),
            SetupEvent::Key(VK_ESCAPE) | SetupEvent::Closed => {
                self.flow.on_action(SetupAction::Close)
            }
            SetupEvent::Key(_) => Vec::new(),
            SetupEvent::Dpi(_) => {
                self.resize();
                Vec::new()
            }
            SetupEvent::Wake => self.drain(),
        };
        self.run_effects(effects);
        self.refresh();
    }

    /// The window has a new size or scale: the renderer follows.
    fn resize(&self) {
        if let (Some(window), Some(render)) = (&self.window, &self.render) {
            let (width, height, scale) = window.client_px();
            render.resize(width, height, scale);
        }
    }

    /// Hands the pump's messages to the flow.
    fn drain(&mut self) -> Vec<plith_core::setup::Effect> {
        let mut effects = Vec::new();
        for msg in take_all(&self.inbox) {
            match msg {
                Msg::Report(report) => effects.extend(self.flow.on_report(report)),
                Msg::Exit(code) => effects.extend(self.flow.on_worker_exit(code)),
                Msg::ElevateFailed { cancelled, text } => {
                    self.log(&format!("worker not started: {text}"));
                    self.flow.on_elevate_failed(cancelled);
                }
                Msg::RenderFatal(text) => {
                    self.log(&format!("render failed: {text}"));
                    self.exit = 1;
                    quit();
                }
            }
        }
        effects
    }

    /// Stops the renderer first (it presents to the window), then destroys the window.
    fn finish(&mut self) {
        if let Some(render) = self.render.take() {
            render.stop_and_join();
        }
        self.window = None;
    }
}
