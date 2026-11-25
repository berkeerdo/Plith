//! These tests write only under `HKCU\Software\Plith-test`, never the real Run key, and delete
//! their subkey (and the parent when it is empty) at the end, also when an assertion fails.

use super::*;
use windows::Win32::System::Registry::RegDeleteKeyW;

const PARENT: &str = r"Software\Plith-test";

/// A unique test subkey that is removed on drop.
struct TestKey {
    path: String,
}

impl TestKey {
    fn new(name: &str) -> Self {
        let path = format!(r"{PARENT}\{name}-{}", std::process::id());
        Self { path }
    }

    fn run_key(&self) -> RunKey {
        RunKey::at(&self.path)
    }
}

impl Drop for TestKey {
    fn drop(&mut self) {
        // SAFETY: plain wide strings, deleting keys of this test under the current user.
        unsafe {
            let _ = RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(self.path.as_str()));
            // Fails while another test's subkey is still there, which is what is wanted.
            let _ = RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(PARENT));
        }
    }
}

#[test]
fn a_missing_key_reads_as_nothing() {
    let key = TestKey::new("missing");
    assert_eq!(key.run_key().read(), Ok(None));
    assert_eq!(key.run_key().delete(), Ok(()));
}

#[test]
fn a_written_value_reads_back_exactly() {
    let key = TestKey::new("roundtrip");
    let text = r#""C:\Program Files\Plith\plith.exe""#;
    key.run_key().write(text).expect("write");
    assert_eq!(key.run_key().read(), Ok(Some(text.to_owned())));
}

#[test]
fn a_value_with_non_ascii_text_reads_back() {
    let key = TestKey::new("unicode");
    let text = "\"C:\\Users\\\u{c7}a\u{11f}r\u{131}\\plith.exe\"";
    key.run_key().write(text).expect("write");
    assert_eq!(key.run_key().read(), Ok(Some(text.to_owned())));
}

#[test]
fn a_write_replaces_and_a_delete_removes() {
    let key = TestKey::new("replace");
    key.run_key().write("one").expect("write");
    key.run_key().write("two").expect("write again");
    assert_eq!(key.run_key().read(), Ok(Some("two".to_owned())));
    key.run_key().delete().expect("delete");
    assert_eq!(key.run_key().read(), Ok(None));
    key.run_key().delete().expect("a second delete is fine");
}

#[test]
fn the_switch_writes_the_quoted_exe_and_reads_it_as_ours() {
    let key = TestKey::new("switch");
    let exe = r"C:\Program Files\Plith\plith.exe".to_owned();
    let mut auto = RegistryAutostart::new(key.run_key(), exe.clone());
    assert!(!auto.is_on());
    auto.set(true).expect("on");
    assert!(auto.is_on());
    assert_eq!(key.run_key().read(), Ok(Some(format!("\"{exe}\""))));
    auto.set(false).expect("off");
    assert!(!auto.is_on());
    assert_eq!(key.run_key().read(), Ok(None));
}

#[test]
fn a_value_of_another_program_reads_as_off_and_a_switch_on_takes_it_over() {
    let key = TestKey::new("foreign");
    key.run_key()
        .write(r#""C:\Other\other.exe""#)
        .expect("write");
    let mut auto = RegistryAutostart::new(key.run_key(), r"D:\plith\plith.exe".to_owned());
    assert!(!auto.is_on());
    auto.set(true).expect("on");
    assert!(auto.is_on());
}

#[test]
fn the_wpf_value_reads_as_on() {
    let key = TestKey::new("wpf");
    key.run_key()
        .write(r#""C:\Program Files\Plith\Plith.exe""#)
        .expect("write");
    let auto = RegistryAutostart::new(key.run_key(), r"D:\plith\plith.exe".to_owned());
    assert!(auto.is_on());
}

#[test]
fn the_real_run_key_can_be_read_without_error() {
    // Read only: the real Run key is never written by a test.
    assert!(RunKey::user_run().read().is_ok());
}
