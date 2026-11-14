//! The text of a caught panic, shared by the render thread and the window guard.

use std::any::Any;

/// The payload of a caught panic as text: a `&str` or `String` message, otherwise
/// `non-string payload`.
#[must_use]
pub fn payload_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        String::from("non-string payload")
    }
}

#[cfg(test)]
mod tests;
