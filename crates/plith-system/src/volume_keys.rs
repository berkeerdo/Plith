//! The volume keys as an event: a low level keyboard hook that only classifies and enqueues.
//!
//! A `WH_KEYBOARD_LL` hook runs for every keystroke on the machine, games included, so a slow
//! proc lags typing everywhere. The proc therefore does one thing: [`handle_key_event`] checks
//! the key code and does a non-blocking channel send, then the proc always calls
//! `CallNextHookEx`. No allocation, lock, log, I/O or window and process query happens inline.

#[cfg(windows)]
mod hook;
#[cfg(test)]
mod tests;

use std::sync::mpsc::SyncSender;
use std::time::Instant;

use crate::flyout::SuppressionWindow;

#[cfg(windows)]
pub use hook::VolumeKeyHook;

/// `VK_VOLUME_MUTE`.
pub const VK_VOLUME_MUTE: u32 = 0xAD;
/// `VK_VOLUME_DOWN`.
pub const VK_VOLUME_DOWN: u32 = 0xAE;
/// `VK_VOLUME_UP`.
pub const VK_VOLUME_UP: u32 = 0xAF;

/// `WM_KEYDOWN`.
const WM_KEYDOWN: usize = 0x0100;
/// `WM_SYSKEYDOWN`.
const WM_SYSKEYDOWN: usize = 0x0104;

/// The whole decision of the hook proc. For a real hook code, a key down and a volume key it
/// opens `window` (when given) from `now()` and sends one `()`. The window open is one relaxed
/// atomic store and the send never blocks (a full or disconnected channel just drops the
/// signal), so the proc stays enqueue-only: no allocation, lock, log or I/O. `now` is only
/// called for a volume key, so other keystrokes never read the clock.
pub fn handle_key_event(
    n_code: i32,
    w_param: usize,
    vk: u32,
    tx: &SyncSender<()>,
    window: Option<&SuppressionWindow>,
    now: impl FnOnce() -> Instant,
) {
    let key_down = w_param == WM_KEYDOWN || w_param == WM_SYSKEYDOWN;
    if n_code >= 0 && key_down && matches!(vk, VK_VOLUME_MUTE | VK_VOLUME_DOWN | VK_VOLUME_UP) {
        if let Some(window) = window {
            window.open(now());
        }
        let _ = tx.try_send(());
    }
}
