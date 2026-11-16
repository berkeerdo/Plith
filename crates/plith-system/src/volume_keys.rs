//! The volume keys as an event: a low level keyboard hook that only classifies and enqueues.
//!
//! A `WH_KEYBOARD_LL` hook runs for every keystroke on the machine, games included, so a slow
//! proc lags typing everywhere. The proc therefore does one thing: [`handle_key_event`] checks
//! the key code and does a non-blocking channel send, then the proc always calls
//! `CallNextHookEx`. Held keys auto-repeat (135 events for one hold were measured), so a repeat
//! opens the flyout suppression window but sends no signal. No allocation, lock, log, I/O or window and process query happens inline.

#[cfg(windows)]
mod hook;
#[cfg(test)]
mod tests;

use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

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
/// `WM_KEYUP`.
const WM_KEYUP: usize = 0x0101;
/// `WM_SYSKEYDOWN`.
const WM_SYSKEYDOWN: usize = 0x0104;
/// `WM_SYSKEYUP`.
const WM_SYSKEYUP: usize = 0x0105;

/// A held key whose last down is older than this counts as released: its key up was lost
/// (a hook timeout), and the next down is a new press.
const STALE_HOLD: Duration = Duration::from_millis(1500);

/// Which volume key an event was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeKey {
    /// `VK_VOLUME_MUTE`.
    Mute,
    /// `VK_VOLUME_DOWN`.
    Down,
    /// `VK_VOLUME_UP`.
    Up,
}

impl VolumeKey {
    fn from_vk(vk: u32) -> Option<Self> {
        match vk {
            VK_VOLUME_MUTE => Some(Self::Mute),
            VK_VOLUME_DOWN => Some(Self::Down),
            VK_VOLUME_UP => Some(Self::Up),
            _ => None,
        }
    }
}

/// Per key, when its last down arrived while it is held; used to collapse auto-repeat.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeldKeys {
    mute: Option<Instant>,
    down: Option<Instant>,
    up: Option<Instant>,
}

impl HeldKeys {
    fn slot(&mut self, key: VolumeKey) -> &mut Option<Instant> {
        match key {
            VolumeKey::Mute => &mut self.mute,
            VolumeKey::Down => &mut self.down,
            VolumeKey::Up => &mut self.up,
        }
    }
}

/// The whole decision of the hook proc. For a real hook code and a volume key:
/// - a down opens `window` (when given) from `now()`, every time, so the flyout stays
///   suppressed for the whole hold. It sends the key only for a new press: the key was not
///   held, or its previous down is older than [`STALE_HOLD`] (a lost key up). Repeats send
///   nothing.
/// - an up clears the key; nothing is sent and the window is not opened.
///
/// The window open is one relaxed atomic store and the send never blocks (a full or
/// disconnected channel just drops the signal), so the proc stays enqueue-only: no
/// allocation, lock, log or I/O. `now` is called at most once, and only for a volume key down.
#[allow(clippy::too_many_arguments)] // the hook proc's whole input, kept flat on purpose
pub fn handle_key_event(
    n_code: i32,
    w_param: usize,
    vk: u32,
    held: &mut HeldKeys,
    tx: &SyncSender<VolumeKey>,
    window: Option<&SuppressionWindow>,
    now: impl FnOnce() -> Instant,
) {
    if n_code < 0 {
        return;
    }
    let Some(key) = VolumeKey::from_vk(vk) else {
        return;
    };
    match w_param {
        WM_KEYUP | WM_SYSKEYUP => *held.slot(key) = None,
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let at = now();
            if let Some(window) = window {
                window.open(at);
            }
            let slot = held.slot(key);
            let new_press = slot.is_none_or(|prev| at.saturating_duration_since(prev) > STALE_HOLD);
            *slot = Some(at);
            if new_press {
                let _ = tx.try_send(key);
            }
        }
        _ => {}
    }
}
