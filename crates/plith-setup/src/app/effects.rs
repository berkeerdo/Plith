//! What the flow asks the window to do outside the window: start the worker, write the Run
//! value, start Plith, delete the user's data.

use super::App;
use super::pump::spawn;
use plith_core::setup::{Effect, Job};
use plith_host::autostart::{Autostart, RegistryAutostart, RunKey};
use plith_host::log::data_dir;
use plith_shell::setup_window::quit;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

/// The file name of the app under the install folder.
const APP_EXE: &str = "plith.exe";
/// The folder under `%LOCALAPPDATA%` that holds the user's data and `setup.log`.
const DATA_FOLDER: &str = "Plith";

impl App {
    /// Carries out the effects in order.
    pub(super) fn run_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Elevate(job) => self.elevate(job),
                Effect::StartPlith => self.start_plith(),
                Effect::SetAutostart(on) => self.set_autostart(on),
                Effect::DeleteUserData => self.delete_user_data(),
                Effect::Log(text) => self.log(&text),
                Effect::Quit => quit(),
            }
        }
    }

    /// Starts the pump for `job`; the answer comes back through the inbox.
    fn elevate(&mut self, job: Job) {
        // The previous pump ended with its last message; this only collects its thread.
        if let Some(previous) = self.pump.take() {
            let _ = previous.join();
        }
        let Some(window) = &self.window else { return };
        match spawn(job, window.hwnd(), self.inbox.clone()) {
            Ok(handle) => self.pump = Some(handle),
            Err(error) => {
                self.log(&format!("cannot start the pump: {error}"));
                self.flow.on_elevate_failed(false);
            }
        }
    }

    /// Starts the installed app from this medium process, never elevated.
    fn start_plith(&self) {
        let Some(root) = &self.root else {
            self.log("cannot start Plith: no install folder");
            return;
        };
        if let Err(error) = open_file(&root.join(APP_EXE), root) {
            self.log(&format!("cannot start Plith: {error}"));
        }
    }

    /// Turns the Run value on (pointing at the installed app) or off.
    fn set_autostart(&self, on: bool) {
        let result = match &self.root {
            Some(root) => {
                let exe = root.join(APP_EXE).display().to_string();
                RegistryAutostart::new(RunKey::user_run(), exe).set(on)
            }
            None if !on => RunKey::user_run().delete(),
            None => Err("no install folder".to_string()),
        };
        if let Err(error) = result {
            self.log(&format!("cannot set start with Windows to {on}: {error}"));
        }
    }

    /// Deletes `%LOCALAPPDATA%\Plith`, after this window has closed its own `setup.log`.
    fn delete_user_data(&mut self) {
        // Close the log first: it lives in the folder that is about to go.
        self.log = None;
        let Ok(dir) = data_dir() else { return };
        // Only the one known folder, never anything else a bad path could name.
        if dir.file_name().is_some_and(|name| name == DATA_FOLDER) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// `ShellExecuteExW` with the `open` verb on `file`, started in `folder`.
fn open_file(file: &Path, folder: &Path) -> Result<(), String> {
    let wide = |path: &Path| -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let (file, folder) = (wide(file), wide(folder));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpDirectory: PCWSTR(folder.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised; `file` and `folder` outlive the call.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|error| error.to_string())
}
