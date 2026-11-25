//! The file checks read installed or freshly built files and write only into a temp folder. The
//! store test writes only to the `CurrentUser` system store `PlithTest`, never to a `LocalMachine`
//! store or the real `CurrentUser` Root, and deletes that store at the end.

use super::store::{Scope, Store, add_cert, has_cert, remove_cert};
use super::*;
use std::path::PathBuf;
use windows::Win32::Foundation::{TRUST_E_BAD_DIGEST, TRUST_E_EXPLICIT_DISTRUST};
use windows::Win32::Security::Cryptography::{
    CERT_OPEN_STORE_FLAGS, CERT_QUERY_ENCODING_TYPE, CERT_STORE_DELETE_FLAG,
    CERT_STORE_PROV_SYSTEM_W, CERT_SYSTEM_STORE_CURRENT_USER, CertOpenStore,
};
use windows::core::HSTRING;

const HELPER: &str = r"C:\Program Files\Plith\osd\plith-osd.exe";
/// Files Windows ships with an embedded Microsoft signature; the first one present is used.
const FOREIGN_SIGNED: [&str; 2] = [
    r"C:\Windows\System32\MRT.exe",
    r"C:\Windows\System32\MpSigStub.exe",
];
const TEST_STORE: &str = "PlithTest";
/// Far past the PE headers, inside the first section.
const TAMPER_OFFSET: usize = 0x1000;

/// A temp folder of this test, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("plith-trust-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deletes the test store on drop, also when an assertion fails.
struct TestStore;

impl Drop for TestStore {
    fn drop(&mut self) {
        let _ = delete_test_store();
    }
}

/// Deletes the `CurrentUser` system store `PlithTest`; true when Windows reports success.
fn delete_test_store() -> bool {
    let name = HSTRING::from(TEST_STORE);
    let flags = CERT_OPEN_STORE_FLAGS(CERT_SYSTEM_STORE_CURRENT_USER) | CERT_STORE_DELETE_FLAG;
    // SAFETY: `name` is a NUL-terminated wide string; with the delete flag no handle is returned.
    let result = unsafe {
        CertOpenStore(
            CERT_STORE_PROV_SYSTEM_W,
            CERT_QUERY_ENCODING_TYPE(0),
            None,
            flags,
            Some(name.as_ptr().cast()),
        )
    };
    // A successful delete returns no handle and leaves the last error at zero.
    match result {
        Ok(_) => true,
        Err(error) => error.code().is_ok(),
    }
}

#[test]
fn unsigned_file_is_not_signed() {
    let dir = TempDir::new("unsigned");
    let copy = dir.0.join("unsigned.exe");
    std::fs::copy(std::env::current_exe().expect("test exe"), &copy).expect("copy");
    assert_eq!(verify_plith(&copy), Err(SignError::NotSigned));
}

#[test]
fn missing_file_is_an_os_error() {
    let dir = TempDir::new("missing");
    let result = verify_plith(&dir.0.join("nothing.exe"));
    assert!(matches!(result, Err(SignError::Os(_))), "{result:?}");
}

#[test]
fn installed_helper_is_plith_signed() {
    let helper = Path::new(HELPER);
    if !helper.exists() {
        println!("skipped: {HELPER} is not installed");
        return;
    }
    assert_eq!(verify_plith(helper), Ok(()));
}

#[test]
fn tampered_signed_file_is_bad_signature() {
    let helper = Path::new(HELPER);
    if !helper.exists() {
        println!("skipped: {HELPER} is not installed");
        return;
    }
    let dir = TempDir::new("tampered");
    let copy = dir.0.join("plith-osd.exe");
    let mut bytes = std::fs::read(helper).expect("read helper");
    assert!(bytes.len() > 2 * TAMPER_OFFSET, "helper is too small");
    bytes[TAMPER_OFFSET] ^= 0xFF;
    std::fs::write(&copy, &bytes).expect("write copy");
    assert_eq!(verify_plith(&copy), Err(SignError::BadSignature));
}

#[test]
fn foreign_signed_file_is_wrong_signer() {
    let Some(foreign) = FOREIGN_SIGNED
        .iter()
        .map(Path::new)
        .find(|path| path.exists())
    else {
        println!("skipped: no file with an embedded Microsoft signature found");
        return;
    };
    let result = verify_plith(foreign);
    assert!(
        matches!(&result, Err(SignError::WrongSigner(thumb)) if thumb.len() == 40 && thumb != PLITH_THUMBPRINT),
        "{}: {result:?}",
        foreign.display()
    );
}

#[test]
fn codes_map_to_verdicts() {
    assert_eq!(classify(HRESULT(0)), Verdict::CheckSigner);
    assert_eq!(classify(CERT_E_UNTRUSTEDROOT), Verdict::CheckSigner);
    assert_eq!(classify(CERT_E_CHAINING), Verdict::CheckSigner);
    assert_eq!(
        classify(TRUST_E_NOSIGNATURE),
        Verdict::Fail(SignError::NotSigned)
    );
    assert_eq!(
        classify(TRUST_E_BAD_DIGEST),
        Verdict::Fail(SignError::BadSignature)
    );
    assert_eq!(
        classify(TRUST_E_EXPLICIT_DISTRUST),
        Verdict::Fail(SignError::BadSignature)
    );
    assert!(matches!(
        classify(CRYPT_E_FILE_ERROR),
        Verdict::Fail(SignError::Os(_))
    ));
    assert!(matches!(
        classify(HRESULT::from_win32(2)),
        Verdict::Fail(SignError::Os(_))
    ));
}

#[test]
fn store_add_has_remove_round_trip() {
    let _cleanup = TestStore;
    let scope = Scope::CurrentUserTest(TEST_STORE);
    assert_eq!(has_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(false));
    assert_eq!(remove_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(()));
    assert_eq!(add_cert(scope, Store::Root, PLITH_CERT), Ok(()));
    assert_eq!(has_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(true));
    assert_eq!(add_cert(scope, Store::Root, PLITH_CERT), Ok(()));
    assert_eq!(has_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(true));
    assert_eq!(remove_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(()));
    assert_eq!(has_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(false));
    assert_eq!(remove_cert(scope, Store::Root, PLITH_THUMBPRINT), Ok(()));
    assert!(add_cert(scope, Store::Root, b"not a certificate").is_err());
    assert!(has_cert(scope, Store::Root, "not hex").is_err());
    assert!(delete_test_store(), "test store not deleted");
}
