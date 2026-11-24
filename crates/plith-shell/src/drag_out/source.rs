//! The drop source of a drag out: the button decides when the drag ends, Escape cancels, the
//! system cursors stay. A panic cancels the drag instead of unwinding into OLE.

// The `#[implement]` expansion trips these pedantic lints; the code it generates is not ours.
#![allow(clippy::ref_as_ptr, clippy::inline_always)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, S_OK,
};
use windows::Win32::System::Ole::{DROPEFFECT, IDropSource, IDropSource_Impl};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::core::{BOOL, HRESULT, implement};

/// The drop source.
#[implement(IDropSource, Agile = false)]
pub(super) struct DropSource;

/// What to answer for the key state: cancel on Escape, drop when the button went up.
fn verdict(escape: bool, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
    if escape {
        DRAGDROP_S_CANCEL
    } else if keys.0 & MK_LBUTTON.0 == 0 {
        DRAGDROP_S_DROP
    } else {
        S_OK
    }
}

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        catch_unwind(AssertUnwindSafe(|| {
            verdict(fescapepressed.as_bool(), grfkeystate)
        }))
        .unwrap_or(DRAGDROP_S_CANCEL)
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_cancels_and_a_released_button_drops() {
        assert_eq!(verdict(true, MK_LBUTTON), DRAGDROP_S_CANCEL);
        assert_eq!(verdict(false, MODIFIERKEYS_FLAGS(0)), DRAGDROP_S_DROP);
        assert_eq!(verdict(false, MK_LBUTTON), S_OK);
    }
}
