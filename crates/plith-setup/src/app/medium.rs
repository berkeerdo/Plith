//! Keeping the setup window at medium integrity. A setup started elevated (WPF's update button
//! uses `runas`, or "Run as administrator") would start Plith elevated, which must never happen.
//! Such a window starts itself again through the desktop shell, which runs as the signed-in user
//! at medium, and exits. When that cannot be done the window stays, but refuses to start Plith.

#[cfg(test)]
mod tests;

use plith_core::setup::quote_arg;
use plith_ipc::security::{own_integrity_above_medium, process_integrity_above_medium};
use std::mem::ManuallyDrop;
use std::path::Path;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, IServiceProvider};
use windows::Win32::System::Variant::{
    VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BSTR, VT_I4, VariantClear,
};
use windows::Win32::UI::Shell::{
    IShellBrowser, IShellDispatch2, IShellFolderViewDual, IShellWindows, SID_STopLevelBrowser,
    SVGIO_BACKGROUND, SWC_DESKTOP, SWFO_NEEDDISPATCH, ShellWindows,
};
use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};
use windows::core::{BSTR, Interface};

/// `SW_SHOWNORMAL`, as the shell's `ShellExecute` takes it.
const SHOW_NORMAL: i32 = 1;

/// What to do about this process's integrity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Plan {
    /// Medium already: go on.
    Medium,
    /// Elevated under a medium shell: start again through the shell and exit.
    Relaunch,
    /// Elevated (or unknown) with no medium shell to hand over to: stay, never start Plith.
    Elevated(String),
}

/// Pure. The plan from this process's integrity and, only when it is above medium, the
/// shell's. A shell that is itself elevated would start an elevated copy again (a loop), so the
/// window then stays.
pub(super) fn plan(
    own: Result<bool, String>,
    shell: impl FnOnce() -> Result<bool, String>,
) -> Plan {
    match own {
        Ok(false) => Plan::Medium,
        Ok(true) => match shell() {
            Ok(false) => Plan::Relaunch,
            Ok(true) => Plan::Elevated("the desktop shell runs elevated too".to_string()),
            Err(reason) => Plan::Elevated(format!("the desktop shell cannot be read: {reason}")),
        },
        Err(reason) => Plan::Elevated(format!("own integrity cannot be read: {reason}")),
    }
}

/// How this window starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Start {
    /// At medium: everything is allowed.
    Medium,
    /// A medium copy was started; this process exits.
    Relaunched,
    /// Elevated and staying so; the reason is logged and `StartPlith` is refused.
    Elevated(String),
}

/// Checks the integrity and, when elevated under a medium shell, starts this exe again with the
/// same arguments through the shell. COM must be initialised on this thread.
pub(super) fn ensure() -> Start {
    match plan(own_integrity_above_medium(), shell_above_medium) {
        Plan::Medium => Start::Medium,
        Plan::Relaunch => match relaunch() {
            Ok(()) => Start::Relaunched,
            Err(reason) => Start::Elevated(format!("cannot start again at medium: {reason}")),
        },
        Plan::Elevated(reason) => Start::Elevated(reason),
    }
}

/// Whether the process that owns the desktop window runs above medium.
fn shell_above_medium() -> Result<bool, String> {
    // SAFETY: plain window lookups; `pid` is a valid out pointer.
    let pid = unsafe {
        let window = GetShellWindow();
        if window.is_invalid() {
            return Err("no desktop shell window".to_string());
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, Some(&raw mut pid));
        pid
    };
    if pid == 0 {
        return Err("no desktop shell process".to_string());
    }
    process_integrity_above_medium(pid)
}

/// The arguments after the program name, quoted again for a command line.
pub(super) fn command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A `VARIANT` that clears what it holds when dropped.
struct Owned(VARIANT);

impl Owned {
    fn tagged(vt: VARENUM, value: VARIANT_0_0_0) -> Self {
        Self(VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                    vt,
                    Anonymous: value,
                    ..Default::default()
                }),
            },
        })
    }

    fn text(text: &str) -> Self {
        let bstr = ManuallyDrop::new(BSTR::from(text));
        Self::tagged(VT_BSTR, VARIANT_0_0_0 { bstrVal: bstr })
    }

    fn int(number: i32) -> Self {
        Self::tagged(VT_I4, VARIANT_0_0_0 { lVal: number })
    }
}

/// Names the failed step of the hand-over.
fn step(what: &'static str) -> impl Fn(windows::core::Error) -> String {
    move |error| format!("{what}: {error}")
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the VARIANT was built by this type and is cleared once.
        let _ = unsafe { VariantClear(&raw mut self.0) };
    }
}

/// Starts this exe with the same arguments through the desktop shell (`IShellWindows` ->
/// desktop browser -> its view -> `IShellDispatch2::ShellExecute`), so it runs as the shell
/// does: the signed-in user at medium.
fn relaunch() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| format!("this exe: {error}"))?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let folder = exe.parent().unwrap_or(Path::new(""));
    let file = BSTR::from(exe.to_string_lossy().as_ref());
    let args = Owned::text(&command_line(&args));
    let folder = Owned::text(&folder.to_string_lossy());
    let verb = Owned::text("open");
    let show = Owned::int(SHOW_NORMAL);
    // SAFETY: COM is initialised on this thread; every pointer comes from the call before it.
    unsafe {
        let windows: IShellWindows =
            CoCreateInstance(&ShellWindows, None, CLSCTX_ALL).map_err(step("ShellWindows"))?;
        let empty = VARIANT::default();
        let mut hwnd = 0i32;
        let desktop = windows
            .FindWindowSW(
                &raw const empty,
                &raw const empty,
                SWC_DESKTOP,
                &raw mut hwnd,
                SWFO_NEEDDISPATCH,
            )
            .map_err(step("FindWindowSW"))?;
        let provider: IServiceProvider = desktop.cast().map_err(step("IServiceProvider"))?;
        let browser: IShellBrowser = provider
            .QueryService(&SID_STopLevelBrowser)
            .map_err(step("IShellBrowser"))?;
        let view = browser.QueryActiveShellView().map_err(step("IShellView"))?;
        let folder_view: IShellFolderViewDual = view
            .GetItemObject(SVGIO_BACKGROUND)
            .map_err(step("IShellFolderViewDual"))?;
        let shell: IShellDispatch2 = folder_view
            .Application()
            .and_then(|app| app.cast())
            .map_err(step("IShellDispatch2"))?;
        shell
            .ShellExecute(&file, &args.0, &folder.0, &verb.0, &show.0)
            .map_err(step("ShellExecute"))
    }
}
