//! The system certificate stores the setup writes the Plith certificate into, matched by
//! thumbprint. Tests use a `CurrentUser` store of their own; nothing here opens the real
//! `CurrentUser` Root, where an add would ask the user.

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::Security::Cryptography::{
    CERT_CONTEXT, CERT_FIND_SHA1_HASH, CERT_OPEN_STORE_FLAGS, CERT_QUERY_ENCODING_TYPE,
    CERT_STORE_ADD_USE_EXISTING, CERT_STORE_OPEN_EXISTING_FLAG, CERT_STORE_PROV_SYSTEM_W,
    CERT_STORE_READONLY_FLAG, CERT_SYSTEM_STORE_CURRENT_USER, CERT_SYSTEM_STORE_LOCAL_MACHINE,
    CRYPT_INTEGER_BLOB, CertAddEncodedCertificateToStore, CertCloseStore,
    CertDeleteCertificateFromStore, CertFindCertificateInStore, CertFreeCertificateContext,
    CertOpenStore, HCERTSTORE, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
};
use windows::core::{HRESULT, HSTRING};

/// A removal stops after this many copies, so a store that keeps returning one cannot spin.
const MAX_COPIES: usize = 16;

/// Which system store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// Trusted root authorities.
    Root,
    /// Trusted publishers, so signed Plith files are trusted without a prompt.
    TrustedPublisher,
}

impl Store {
    fn name(self) -> &'static str {
        match self {
            Self::Root => "Root",
            Self::TrustedPublisher => "TrustedPublisher",
        }
    }
}

/// Where the store lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The machine's store; writing needs elevation.
    LocalMachine,
    /// A `CurrentUser` system store with this name, for tests. The [`Store`] kind is ignored:
    /// every kind maps to this one store.
    CurrentUserTest(&'static str),
}

/// Whether a certificate with `thumbprint` is in the store. A missing store has none.
///
/// # Errors
/// The text of the error when the thumbprint is not 40 hex digits or the store cannot be read.
pub fn has_cert(scope: Scope, store: Store, thumbprint: &str) -> Result<bool, String> {
    let hash = parse_thumbprint(thumbprint)?;
    let flags = CERT_STORE_READONLY_FLAG | CERT_STORE_OPEN_EXISTING_FLAG;
    let Some(open) = OpenStore::open(scope, store, flags)? else {
        return Ok(false);
    };
    let found = open.find(&hash);
    if found.is_null() {
        return Ok(false);
    }
    // SAFETY: `found` is a context this call got from the find and frees once.
    let _ = unsafe { CertFreeCertificateContext(Some(found)) };
    Ok(true)
}

/// Adds the DER certificate `der` to the store; an existing copy is kept as it is.
///
/// # Errors
/// The text of the error when the store cannot be opened or written, or `der` is not a
/// certificate.
pub fn add_cert(scope: Scope, store: Store, der: &[u8]) -> Result<(), String> {
    let open = OpenStore::open(scope, store, CERT_OPEN_STORE_FLAGS(0))?
        .ok_or_else(|| format!("store {} not found", store.name()))?;
    // SAFETY: the store is open; `der` is read for the length of the slice only.
    unsafe {
        CertAddEncodedCertificateToStore(
            Some(open.0),
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            der,
            CERT_STORE_ADD_USE_EXISTING,
            None,
        )
    }
    .map_err(|error| format!("add certificate: {error}"))
}

/// Removes every copy of the certificate with `thumbprint` from the store; none is fine.
///
/// # Errors
/// The text of the error when the thumbprint is not 40 hex digits, or the store cannot be opened
/// or written.
pub fn remove_cert(scope: Scope, store: Store, thumbprint: &str) -> Result<(), String> {
    let hash = parse_thumbprint(thumbprint)?;
    let Some(open) = OpenStore::open(scope, store, CERT_STORE_OPEN_EXISTING_FLAG)? else {
        return Ok(());
    };
    for _ in 0..MAX_COPIES {
        let found = open.find(&hash);
        if found.is_null() {
            return Ok(());
        }
        // SAFETY: `found` came from the find; the delete frees it whether or not it succeeds.
        unsafe { CertDeleteCertificateFromStore(found) }
            .map_err(|error| format!("delete certificate: {error}"))?;
    }
    Err(format!(
        "store {} keeps returning the certificate",
        store.name()
    ))
}

/// An open system store, closed on drop.
struct OpenStore(HCERTSTORE);

impl OpenStore {
    /// Opens the store; `None` when `flags` asks for an existing store and there is none.
    fn open(
        scope: Scope,
        store: Store,
        flags: CERT_OPEN_STORE_FLAGS,
    ) -> Result<Option<Self>, String> {
        let (location, name) = match scope {
            Scope::LocalMachine => (CERT_SYSTEM_STORE_LOCAL_MACHINE, store.name()),
            Scope::CurrentUserTest(name) => (CERT_SYSTEM_STORE_CURRENT_USER, name),
        };
        let wide = HSTRING::from(name);
        // SAFETY: the system provider takes a NUL-terminated wide store name, alive for the call.
        let result = unsafe {
            CertOpenStore(
                CERT_STORE_PROV_SYSTEM_W,
                CERT_QUERY_ENCODING_TYPE(0),
                None,
                CERT_OPEN_STORE_FLAGS(location) | flags,
                Some(wide.as_ptr().cast()),
            )
        };
        match result {
            Ok(handle) => Ok(Some(Self(handle))),
            Err(error)
                if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
                    && flags.contains(CERT_STORE_OPEN_EXISTING_FLAG) =>
            {
                Ok(None)
            }
            Err(error) => Err(format!("open store {name}: {error}")),
        }
    }

    /// The first certificate whose SHA-1 hash is `hash`, or null. The caller frees it.
    fn find(&self, hash: &[u8; 20]) -> *mut CERT_CONTEXT {
        let blob = CRYPT_INTEGER_BLOB {
            cbData: 20,
            pbData: hash.as_ptr().cast_mut(),
        };
        // SAFETY: the store is open; `blob` and the hash it points to outlive the call, which
        // only reads them.
        unsafe {
            CertFindCertificateInStore(
                self.0,
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                0,
                CERT_FIND_SHA1_HASH,
                Some(std::ptr::from_ref(&blob).cast()),
                None,
            )
        }
    }
}

impl Drop for OpenStore {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful open and is closed once, here.
        let _ = unsafe { CertCloseStore(Some(self.0), 0) };
    }
}

/// 40 hex digits (either case) into the 20 bytes of a SHA-1 hash.
fn parse_thumbprint(text: &str) -> Result<[u8; 20], String> {
    let bad = || format!("not a thumbprint: {text}");
    if text.len() != 40 || !text.is_ascii() {
        return Err(bad());
    }
    let mut hash = [0u8; 20];
    for (byte, pair) in hash.iter_mut().zip(text.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| bad())?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| bad())?;
    }
    Ok(hash)
}
