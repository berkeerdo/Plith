//! A file drag into the island (ADR 0018): the `IDropTarget` registered on the notch window.
//!
//! OLE calls the target on the UI thread, in the message loop. Each call tests the data object
//! for a file list (`CF_HDROP`), answers the drop effect (copy, link with Alt, never move),
//! lets the shell draw the drag image through its helper, and reports to the app as a
//! [`ShellEvent`]. Anything else, and the island's own drag out, gets no effect. Every COM body
//! runs inside a panic guard: a panic must not unwind into OLE.

// The `#[implement]` expansion trips these pedantic lints; the code it generates is not ours.
#![allow(clippy::ref_as_ptr, clippy::inline_always)]

mod data;

use crate::drag_out::effect_word;
use crate::handles::hwnd_of;
use crate::window::mouse::island_point_unclamped;
use crate::window::state::{Inner, current};
use crate::window::{ShellEvent, guard};
use std::cell::Cell;
use windows::Win32::Foundation::{E_FAIL, HWND, POINT, POINTL};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IDataObject};
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
    MK_ALT, RegisterDragDrop, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{CLSID_DragDropHelper, IDropTargetHelper};
use windows::core::{Ref, Result, implement};

/// The drop target of the notch window.
#[implement(IDropTarget, Agile = false)]
struct DropTarget {
    hwnd: isize,
    /// The shell's drag image helper; `None` when it could not be created (no drag image).
    helper: Option<IDropTargetHelper>,
    /// The drag now over the island offers a file list and is not our own.
    accepted: Cell<bool>,
    /// The last client point reported, so a drag at rest emits nothing.
    last: Cell<Option<(i32, i32)>>,
}

/// Runs `body` inside the panic guard. A panic is reported to the shell (which closes the
/// window like any callback panic) and answered with a failure code.
fn guarded(label: &str, body: impl FnOnce() -> Result<()>) -> Result<()> {
    let mut outcome = Ok(());
    match guard(label, || outcome = body()) {
        Ok(()) => outcome,
        Err(message) => {
            if let Some(inner) = current() {
                inner.fail(message);
            }
            Err(E_FAIL.into())
        }
    }
}

/// The drop effect to answer: copy, link with Alt, and only what the source allows.
fn effect_for(keys: MODIFIERKEYS_FLAGS, allowed: DROPEFFECT) -> DROPEFFECT {
    let wanted = if keys.0 & MK_ALT != 0 {
        DROPEFFECT_LINK
    } else {
        DROPEFFECT_COPY
    };
    if allowed & wanted == wanted {
        wanted
    } else if allowed & DROPEFFECT_COPY == DROPEFFECT_COPY {
        DROPEFFECT_COPY
    } else if allowed & DROPEFFECT_LINK == DROPEFFECT_LINK {
        DROPEFFECT_LINK
    } else {
        DROPEFFECT_NONE
    }
}

impl DropTarget_Impl {
    /// The client pixel point and the island DIP point of a screen point.
    fn locate(&self, inner: &Inner, pt: &POINTL) -> ((i32, i32), (f64, f64)) {
        let mut at = POINT { x: pt.x, y: pt.y };
        // SAFETY: names the notch window of this thread; `at` is a valid POINT for the call.
        // A failure leaves the screen point, which only shifts the reported position.
        let _ = unsafe { ScreenToClient(hwnd_of(self.hwnd), &raw mut at) };
        let island = inner.shapes.latest().map_or((0.0, 0.0), |frame| {
            island_point_unclamped(
                &inner.layout(),
                frame.shape,
                (f64::from(at.x), f64::from(at.y)),
            )
        });
        ((at.x, at.y), island)
    }

    fn enter(
        &self,
        data: Option<&IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let (Some(inner), Some(data)) = (current(), data) else {
            return Err(E_FAIL.into());
        };
        let own = inner.dragging_out.get();
        let accepted = !own && data::has_files(data);
        self.accepted.set(accepted);
        self.last.set(None);
        // SAFETY: OLE passes a valid, writable effect for the whole call.
        let granted = if accepted {
            effect_for(keys, unsafe { *effect })
        } else {
            DROPEFFECT_NONE
        };
        // SAFETY: as above.
        unsafe { *effect = granted };
        let (client, (x, y)) = self.locate(&inner, pt);
        if let Some(helper) = &self.helper {
            let at = POINT { x: pt.x, y: pt.y };
            // SAFETY: the helper and the data object are live; `at` is valid for the call.
            // The drag image is a courtesy: a failure only loses it.
            let _ = unsafe { helper.DragEnter(hwnd_of(self.hwnd), data, &raw const at, granted) };
        }
        self.last.set(Some(client));
        if own {
            return Ok(());
        }
        let formats = data::formats(data);
        inner.emit(if accepted {
            ShellEvent::DragEnter {
                x,
                y,
                formats,
                effect: effect_word(granted),
            }
        } else {
            ShellEvent::DragRefused { formats }
        });
        Ok(())
    }

    fn over(&self, keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let Some(inner) = current() else {
            return Err(E_FAIL.into());
        };
        // SAFETY: OLE passes a valid, writable effect for the whole call.
        let granted = if self.accepted.get() {
            effect_for(keys, unsafe { *effect })
        } else {
            DROPEFFECT_NONE
        };
        // SAFETY: as above.
        unsafe { *effect = granted };
        if let Some(helper) = &self.helper {
            let at = POINT { x: pt.x, y: pt.y };
            // SAFETY: the helper is live and `at` is valid for the call.
            let _ = unsafe { helper.DragOver(&raw const at, granted) };
        }
        let (client, (x, y)) = self.locate(&inner, pt);
        if self.accepted.get() && self.last.replace(Some(client)) != Some(client) {
            inner.emit(ShellEvent::DragOver { x, y });
        }
        Ok(())
    }

    fn leave(&self) {
        if let Some(helper) = &self.helper {
            // SAFETY: the helper is live.
            let _ = unsafe { helper.DragLeave() };
        }
        let accepted = self.accepted.replace(false);
        self.last.set(None);
        if let (Some(inner), true) = (current(), accepted) {
            reset_mouse(&inner);
            inner.emit(ShellEvent::DragLeave);
        }
    }

    fn dropped(
        &self,
        data: Option<&IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let (Some(inner), Some(data)) = (current(), data) else {
            return Err(E_FAIL.into());
        };
        let accepted = self.accepted.replace(false);
        self.last.set(None);
        // SAFETY: OLE passes a valid, writable effect for the whole call.
        let granted = if accepted {
            effect_for(keys, unsafe { *effect })
        } else {
            DROPEFFECT_NONE
        };
        // SAFETY: as above.
        unsafe { *effect = granted };
        if let Some(helper) = &self.helper {
            let at = POINT { x: pt.x, y: pt.y };
            // SAFETY: the helper and the data object are live; `at` is valid for the call.
            let _ = unsafe { helper.Drop(data, &raw const at, granted) };
        }
        if !accepted {
            return Ok(());
        }
        reset_mouse(&inner);
        // Ask for the leave now: the first move after a drop may already be outside the
        // island, and then no `Enter` would ever arm it. If the pointer is already out the
        // system posts the leave at once, which is the leave ADR 0018 wants after a drop.
        crate::window::mouse::rearm_leave(&inner);
        let paths = data::files(data);
        inner.emit(if paths.is_empty() {
            ShellEvent::DragLeave
        } else {
            ShellEvent::Drop { paths }
        });
        Ok(())
    }
}

/// OLE owned the mouse for the whole drag and may have swallowed the leave and the release, so
/// the tracking, the last point and any press are forgotten (ADR 0018).
fn reset_mouse(inner: &Inner) {
    inner.tracking.set(false);
    inner.last_point.set(None);
    inner.press.cancel();
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        guarded("drag enter", || {
            self.enter(pdataobj.as_ref(), grfkeystate, pt, pdweffect)
        })
    }

    fn DragOver(
        &self,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        guarded("drag over", || self.over(grfkeystate, pt, pdweffect))
    }

    fn DragLeave(&self) -> Result<()> {
        guarded("drag leave", || {
            self.leave();
            Ok(())
        })
    }

    fn Drop(
        &self,
        pdataobj: Ref<IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        guarded("drop", || {
            self.dropped(pdataobj.as_ref(), grfkeystate, pt, pdweffect)
        })
    }
}

/// Registers the drop target on `hwnd`. The apartment must be an STA with OLE
/// (`init_ui_thread`); the window must be on this thread.
///
/// # Errors
/// Returns the OLE error when the registration fails; the window then takes no drops.
pub(crate) fn register(hwnd: HWND) -> Result<()> {
    // SAFETY: plain COM creation; a missing helper only loses the drag image.
    let helper =
        unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) }.ok();
    let target: IDropTarget = DropTarget {
        hwnd: hwnd.0 as isize,
        helper,
        accepted: Cell::new(false),
        last: Cell::new(None),
    }
    .into();
    // SAFETY: `hwnd` is a live window of this thread; OLE keeps its own reference to `target`.
    unsafe { RegisterDragDrop(hwnd, &target) }
}

/// Takes the drop target off `hwnd`. A window that never had one is not an error worth a name.
pub(crate) fn revoke(hwnd: HWND) {
    // SAFETY: plain call; revoking a window with no target only fails.
    let _ = unsafe { RevokeDragDrop(hwnd) };
}
