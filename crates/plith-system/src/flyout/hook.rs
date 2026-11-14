//! `FlyoutSuppressor`: `WinEvent` hooks (`SHOW`, `CREATE`, `LOCATIONCHANGE`) on their own thread.
//!
//! `LOCATIONCHANGE` fires thousands of times per second, so the callback runs the cheapest
//! filter first (is the suppression window open) and does no I/O, no allocation and no Win32
//! call before it. Window class, process name and band are only looked up after that.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Instant;

use windows::Win32::Foundation::{CloseHandle, FARPROC, HWND};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CREATE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW, GetClassNameW,
    GetWindowThreadProcessId, IsWindowVisible, OBJID_WINDOW, SW_HIDE, SW_SHOWNA, ShowWindowAsync,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};
use windows::core::{BOOL, PWSTR, s, w};

use super::{
    HiddenRegistry, Match, Mode, SuppressionWindow, WindowFacts, is_host_class, should_hide,
};
use crate::hook_thread::HookThread;

/// Matches queued for the owner before new ones are dropped.
const QUEUE: usize = 64;
/// The pid to process name cache is cleared when it grows past this (pid reuse, long runs).
const CACHE_CAP: usize = 512;

type GetWindowBand = unsafe extern "system" fn(HWND, *mut u32) -> BOOL;

/// Everything the callback touches; lives in a thread local on the hook thread, so no lock.
struct State {
    mode: Mode,
    window: Arc<SuppressionWindow>,
    registry: HiddenRegistry,
    names: HashMap<u32, String>,
    band_fn: Option<GetWindowBand>,
    matches: SyncSender<Match>,
}

impl Drop for State {
    /// Runs on the hook thread when it ends, also when it panics: give back what was hidden.
    fn drop(&mut self) {
        self.registry.restore_all(|hwnd| {
            // SAFETY: `ShowWindowAsync` on a possibly dead handle fails harmlessly. It posts the
            // request to the window's own thread, so a hung Explorer cannot block this thread
            // (and with it `stop`). SHOWNA, never SHOW: the shell parks this window and must not
            // lose or gain activation.
            let _ = unsafe { ShowWindowAsync(HWND(hwnd as *mut _), SW_SHOWNA) };
        });
    }
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// The three installed hooks; unhooked on drop, then the state (and the restore) is dropped.
struct Installed(Vec<HWINEVENTHOOK>);

impl Drop for Installed {
    fn drop(&mut self) {
        for hook in self.0.drain(..) {
            // SAFETY: each handle came from `SetWinEventHook` and is released once.
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        STATE.with(|state| drop(state.borrow_mut().take()));
    }
}

/// Resolves the undocumented `GetWindowBand` export; `None` when this Windows lacks it.
fn resolve_band_fn() -> Option<GetWindowBand> {
    // SAFETY: user32 is always loaded; the name is a valid C string.
    let proc: FARPROC = unsafe {
        let module = GetModuleHandleW(w!("user32.dll")).ok()?;
        GetProcAddress(module, s!("GetWindowBand"))
    };
    // SAFETY: the export has the signature `BOOL GetWindowBand(HWND, PDWORD)`.
    proc.map(|f| unsafe { std::mem::transmute::<_, GetWindowBand>(f) })
}

/// Longest class name read; the host classes are all shorter.
const CLASS_BUF: usize = 128;

/// Reads the class name into `buf` and returns how many UTF-16 units it holds (0 on failure).
fn class_name_into(hwnd: HWND, buf: &mut [u16; CLASS_BUF]) -> usize {
    // SAFETY: `buf` is a valid writable buffer.
    let len = unsafe { GetClassNameW(hwnd, buf) };
    usize::try_from(len).unwrap_or(0)
}

fn process_name(pid: u32) -> String {
    // SAFETY: standard open, query and close; the handle is closed on every path.
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 520];
        let mut len = 520u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut len,
        )
        .is_ok();
        let _ = CloseHandle(process);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let file = path.rsplit(['\\', '/']).next().unwrap_or_default();
        file.strip_suffix(".exe")
            .or_else(|| file.strip_suffix(".EXE"))
            .unwrap_or(file)
            .to_owned()
    }
}

impl State {
    /// Process and band lookups; called only for a window whose class already matched.
    fn facts(&mut self, hwnd: HWND, class: String) -> WindowFacts {
        let mut pid = 0u32;
        // SAFETY: `pid` is a valid out pointer.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
        if self.names.len() >= CACHE_CAP {
            self.names.clear();
        }
        let process = self
            .names
            .entry(pid)
            .or_insert_with(|| process_name(pid))
            .clone();
        let band = self.band_fn.and_then(|get| {
            let mut band = 0u32;
            // SAFETY: resolved export with the documented-by-use signature.
            let ok = unsafe { get(hwnd, &raw mut band) }.as_bool();
            (ok && band != u32::MAX).then_some(band)
        });
        WindowFacts {
            class,
            process,
            band,
        }
    }

    fn on_event(&mut self, hwnd: HWND) {
        // The class check is a stack buffer and a non-allocating compare; the process and band
        // lookups (OpenProcess, a String) only run for the few windows that pass it.
        let mut buf = [0u16; CLASS_BUF];
        let len = class_name_into(hwnd, &mut buf);
        if !is_host_class(&buf[..len]) {
            return;
        }
        let facts = self.facts(hwnd, String::from_utf16_lossy(&buf[..len]));
        if !should_hide(&facts, true) {
            return;
        }
        let key = hwnd.0 as isize;
        let hidden = self.mode == Mode::Hide
            && self.registry.hide(
                key,
                // SAFETY: a plain visibility query on a handle from an event.
                |h| unsafe { IsWindowVisible(HWND(h as *mut _)) }.as_bool(),
                // SAFETY: `ShowWindowAsync(SW_HIDE)` only posts the request to the window's
                // thread, so it cannot block on a hung process; the registry records the window
                // only when it was visible just before.
                |h| unsafe { ShowWindowAsync(HWND(h as *mut _), SW_HIDE) }.as_bool(),
            );
        let _ = self.matches.try_send(Match {
            hwnd: key,
            facts,
            hidden,
        });
    }
}

unsafe extern "system" fn callback(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.0.is_null() || id_object != OBJID_WINDOW.0 || id_child != 0 {
        return;
    }
    STATE.with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else {
            return;
        };
        if let Some(state) = guard.as_mut() {
            // The cheapest filter first: nothing below runs outside the 400 ms window.
            if state.window.is_open(Instant::now()) {
                state.on_event(hwnd);
            }
        }
    });
}

/// Hides the shell's volume flyout for a moment after a volume key (see [`Mode`]). Dropping it
/// (or [`stop`](Self::stop)) unhooks and restores every window it hid with `SW_SHOWNA`.
#[derive(Debug)]
pub struct FlyoutSuppressor {
    window: Arc<SuppressionWindow>,
    matches: Receiver<Match>,
    thread: HookThread,
}

impl FlyoutSuppressor {
    /// Installs the three `WinEvent` hooks on a new message thread.
    ///
    /// # Errors
    /// The text says why the thread or a hook could not start.
    pub fn start(mode: Mode) -> Result<Self, String> {
        let window = Arc::new(SuppressionWindow::new(Instant::now()));
        let (tx, matches) = sync_channel(QUEUE);
        let shared = Arc::clone(&window);
        let thread = HookThread::spawn("plith-flyout", move || {
            STATE.with(|cell| {
                *cell.borrow_mut() = Some(State {
                    mode,
                    window: shared,
                    registry: HiddenRegistry::new(),
                    names: HashMap::new(),
                    band_fn: resolve_band_fn(),
                    matches: tx,
                });
            });
            // Constructed first: if a later hook fails, dropping it unhooks the earlier ones.
            let mut installed = Installed(Vec::new());
            for event in [
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_LOCATIONCHANGE,
            ] {
                // SAFETY: `callback` matches WINEVENTPROC; out of context, so no DLL is needed.
                let hook = unsafe {
                    SetWinEventHook(
                        event,
                        event,
                        None,
                        Some(callback),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    )
                };
                if hook.0.is_null() {
                    return Err(format!("SetWinEventHook failed for event {event:#x}"));
                }
                installed.0.push(hook);
            }
            Ok(installed)
        })?;
        Ok(Self {
            window,
            matches,
            thread,
        })
    }

    /// The shared suppression window, for [`VolumeKeyHook::start`](crate::VolumeKeyHook::start):
    /// the key hook opens it inline, so no consumer thread sits between a key and the window.
    #[must_use]
    pub fn suppression_window(&self) -> Arc<SuppressionWindow> {
        Arc::clone(&self.window)
    }

    /// Opens the suppression window from now (call when a volume key was seen).
    pub fn open_window(&self) {
        self.window.open(Instant::now());
    }

    /// Windows that passed every filter (hidden, or only reported in [`Mode::DryRun`]).
    #[must_use]
    pub fn matches(&self) -> &Receiver<Match> {
        &self.matches
    }

    /// Unhooks, restores every hidden window and joins the thread. Safe to call twice.
    ///
    /// # Errors
    /// The thread could not be told to quit and is still running; it was not joined.
    pub fn stop(&mut self) -> Result<(), String> {
        self.thread.stop()
    }
}

impl Drop for FlyoutSuppressor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
