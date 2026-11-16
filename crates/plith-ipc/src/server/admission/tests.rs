use super::{Rejection, admit};
use crate::{DecodeError, Message};

#[test]
fn hello_with_the_kernel_pid_is_admitted() {
    let hello = Message::Hello { client_pid: 42 };
    assert_eq!(admit(&Ok(hello.clone()), 42), Ok(hello));
}

#[test]
fn hello_with_another_pid_is_refused() {
    assert_eq!(
        admit(&Ok(Message::Hello { client_pid: 7 }), 42),
        Err(Rejection::PidMismatch {
            kernel: 42,
            hello: 7
        })
    );
}

#[test]
fn any_other_first_message_is_refused() {
    for first in [
        Ok(Message::Hide),
        Ok(Message::Bye),
        Err(DecodeError::Magic),
        Err(DecodeError::Trailing),
    ] {
        assert_eq!(admit(&first, 42), Err(Rejection::NotHello));
    }
}

#[test]
fn rejections_read_as_text() {
    let text = Rejection::PidMismatch {
        kernel: 1,
        hello: 2,
    }
    .to_string();
    assert!(text.contains("pid 2") && text.contains("pid 1"), "{text}");
    assert!(Rejection::Timeout.to_string().contains("1000 ms"));
}
