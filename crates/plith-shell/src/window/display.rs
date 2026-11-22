//! Display changes: the notch window follows the primary monitor.
//!
//! `WM_DISPLAYCHANGE` and `WM_DPICHANGED` arrive in bursts (an RDP connect sends several) and
//! can be sent messages, so they only post one `WM_APP_RELAYOUT`. Its handler re-reads the
//! monitor, moves the window, rebuilds the hit region and tells the app, once per real change.

use super::ShellEvent;
use super::place;
use super::state::Inner;
use crate::handles::hwnd_of;
use crate::hit::apply::apply_latest;
use crate::monitor::{MonitorInfo, is_remote_session, primary_monitor};
use plith_core::display::Display;
use plith_core::layout::SurfaceLayout;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

/// Posted to the notch window once per burst of display messages.
pub(crate) const WM_APP_RELAYOUT: u32 = WM_APP + 4;

/// The display the notch is laid out for on `monitor`.
#[must_use]
pub(crate) fn display_for(monitor: MonitorInfo, remote: bool) -> Display {
    Display {
        layout: SurfaceLayout::new(monitor.rect, monitor.work_top_px, monitor.scale),
        monitor: monitor.handle,
        hz: monitor.hz,
        remote,
    }
}

/// The display for `monitor`, or `None` when it equals `current`.
#[must_use]
pub(crate) fn next_display(
    current: Display,
    monitor: MonitorInfo,
    remote: bool,
) -> Option<Display> {
    let next = display_for(monitor, remote);
    (next != current).then_some(next)
}

/// Runs `post` unless a relayout is already queued.
pub(crate) fn request(inner: &Inner, post: impl FnOnce() -> bool) {
    let _ = inner.relayout.post_with(post);
}

/// `WM_DISPLAYCHANGE` or `WM_DPICHANGED` on the notch window: queue one relayout.
pub(crate) fn on_message(inner: &Inner, hwnd: HWND) {
    request(inner, || {
        // SAFETY: posting to a live window of this thread dereferences nothing.
        unsafe { PostMessageW(Some(hwnd), WM_APP_RELAYOUT, WPARAM(0), LPARAM(0)) }.is_ok()
    });
}

/// `WM_APP_RELAYOUT`: lays the notch out for the primary monitor as it is now.
pub(crate) fn on_relayout(inner: &Inner) {
    relayout_to(inner, primary_monitor(), is_remote_session());
}

/// Moves the window to the display for `monitor`, rebuilds the hit region and emits
/// `DisplayChanged`; nothing when the display is unchanged.
pub(crate) fn relayout_to(inner: &Inner, monitor: MonitorInfo, remote: bool) {
    inner.relayout.clear();
    let Some(next) = next_display(inner.display.get(), monitor, remote) else {
        return;
    };
    inner.display.set(next);
    let visual = inner.visual.get();
    if visual != 0 {
        // A failed move leaves the window where it was; the next display change retries.
        let _ = place(hwnd_of(visual), next.layout.surface);
    }
    // The cached shape was applied at the old scale and position.
    inner.applied.set(None);
    apply_latest(inner);
    inner.emit(ShellEvent::DisplayChanged(next));
}

/// A local 60 Hz display for `layout`, for tests that build an `Inner` without a monitor.
#[cfg(test)]
pub(crate) fn test_display(layout: SurfaceLayout) -> Display {
    Display {
        layout,
        monitor: 0,
        hz: 60.0,
        remote: false,
    }
}

#[cfg(test)]
mod tests;
