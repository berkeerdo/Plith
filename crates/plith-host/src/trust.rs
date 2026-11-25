//! Trust in Plith's own files: the Authenticode check that the setup worker and the update
//! download run before they use a file, and the certificate stores the setup writes.
//!
//! "Signed by Plith" means a valid Authenticode signature whose signer leaf has the pinned
//! thumbprint. The certificate is self-signed, so an untrusted root or a broken chain is accepted,
//! but only for that leaf: adding it to the machine's Root store is what the setup does next on a
//! clean machine. Every other failure (a tampered file, a bad signature, a distrusted signer)
//! refuses the file. Revocation is not checked: the certificate has no revocation list.

mod store;
#[cfg(test)]
mod tests;

use std::fmt::{self, Write as _};
use std::path::Path;
use windows::Win32::Foundation::{
    CERT_E_CHAINING, CERT_E_UNTRUSTEDROOT, CRYPT_E_FILE_ERROR, HANDLE, HWND, INVALID_HANDLE_VALUE,
    TRUST_E_NOSIGNATURE, TRUST_E_SUBJECT_FORM_UNKNOWN,
};
use windows::Win32::Security::Cryptography::{
    CERT_CONTEXT, CERT_SHA1_HASH_PROP_ID, CertGetCertificateContextProperty,
};
use windows::Win32::Security::WinTrust::{
    TRUSTERROR_STEP_FINAL_INITPROV, TRUSTERROR_STEP_FINAL_OBJPROV, TRUSTERROR_STEP_FINAL_SIGPROV,
    TRUSTERROR_STEP_FINAL_WVTINIT, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA,
    WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE,
    WTD_DISABLE_MD2_MD4, WTD_REVOCATION_CHECK_NONE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvSignerFromChain,
    WTHelperProvDataFromStateData, WinVerifyTrust,
};
use windows::core::{HRESULT, HSTRING, PCWSTR};

pub use store::{Scope, Store, add_cert, has_cert, remove_cert};

/// SHA-1 thumbprint of the Plith signing certificate, upper-case hex.
pub const PLITH_THUMBPRINT: &str = "144A7919A1FC941FDC3A9443C1DBAA7AE002AC0F";

/// The public part of the Plith signing certificate (DER). Never the private key.
pub const PLITH_CERT: &[u8] = include_bytes!("../assets/plith-public.cer");

/// Why a file is not trusted as Plith's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignError {
    /// The file carries no Authenticode signature.
    NotSigned,
    /// A signature is there but does not hold: the file was changed, or the signature or its
    /// certificate is invalid or distrusted.
    BadSignature,
    /// A valid signature by someone else; the signer's thumbprint.
    WrongSigner(String),
    /// Windows could not check the file (it is missing or unreadable, or a call failed).
    Os(String),
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSigned => f.write_str("not signed"),
            Self::BadSignature => f.write_str("bad signature"),
            Self::WrongSigner(thumb) => write!(f, "signed by {thumb}, not Plith"),
            Self::Os(text) => write!(f, "signature check failed: {text}"),
        }
    }
}

impl std::error::Error for SignError {}

/// What a `WinVerifyTrust` code means before the signer is looked at.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// Valid, or valid but for the root: the signer decides.
    CheckSigner,
    /// Refused whoever the signer is.
    Fail(SignError),
}

/// Sorts a `WinVerifyTrust` code. Only success and a root or chain error reach the signer
/// check; everything else, a digest mismatch first of all, is a refusal.
fn classify(code: HRESULT) -> Verdict {
    if code.is_ok() || code == CERT_E_UNTRUSTEDROOT || code == CERT_E_CHAINING {
        Verdict::CheckSigner
    } else if code == TRUST_E_NOSIGNATURE || code == TRUST_E_SUBJECT_FORM_UNKNOWN {
        Verdict::Fail(SignError::NotSigned)
    } else if code == CRYPT_E_FILE_ERROR || is_win32(code) {
        Verdict::Fail(SignError::Os(code.message()))
    } else {
        Verdict::Fail(SignError::BadSignature)
    }
}

/// A code from the Win32 facility: a system call under the check failed.
fn is_win32(code: HRESULT) -> bool {
    // HRESULT bits reinterpreted to read the facility.
    #[allow(clippy::cast_sign_loss)]
    let bits = code.0 as u32;
    bits & 0xFFFF_0000 == 0x8007_0000
}

/// Checks that `path` carries a valid Authenticode signature by the Plith certificate.
///
/// # Errors
/// [`SignError`] saying why the file is not trusted.
pub fn verify_plith(path: &Path) -> Result<(), SignError> {
    let wide = HSTRING::from(path.as_os_str());
    verify_file(&wide, HANDLE::default())
}

/// Like [`verify_plith`], but judges the file behind an open handle, so the file that was
/// judged is the file the caller then reads from the same handle, whatever happens to the path
/// in between. `path` is only the name Windows records; the bytes come from `file`.
///
/// The file position is not defined afterwards: seek before reading.
///
/// # Errors
/// [`SignError`] saying why the file is not trusted.
pub fn verify_plith_handle(file: &std::fs::File, path: &Path) -> Result<(), SignError> {
    use std::os::windows::io::AsRawHandle;
    let wide = HSTRING::from(path.as_os_str());
    verify_file(&wide, HANDLE(file.as_raw_handle()))
}

/// The one judgement behind both entry points: a null `handle` lets Windows open the path.
fn verify_file(wide: &HSTRING, handle: HANDLE) -> Result<(), SignError> {
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_u32::<WINTRUST_FILE_INFO>(),
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        hFile: handle,
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_u32::<WINTRUST_DATA>(),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_REVOCATION_CHECK_NONE | WTD_DISABLE_MD2_MD4 | WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let code = HRESULT(call_verify(&mut data));
    let verdict = match classify(code) {
        Verdict::Fail(error) => Err(error),
        // SAFETY: the state data comes from the verify call above and is closed only below.
        Verdict::CheckSigner => unsafe { check_signer(data.hWVTStateData) },
    };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    let _ = call_verify(&mut data);
    verdict
}

/// One `WinVerifyTrust` call with the generic Authenticode action and no window.
fn call_verify(data: &mut WINTRUST_DATA) -> i32 {
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: `data` is a filled WINTRUST_DATA whose file info and path outlive the call; no UI
    // is shown, so the window handle is only the "no interactive user" marker.
    unsafe {
        WinVerifyTrust(
            HWND(INVALID_HANDLE_VALUE.0),
            &raw mut action,
            std::ptr::from_mut(data).cast(),
        )
    }
}

/// Accepts the signature when the steps before the chain check found nothing and the signer's
/// leaf has the pinned thumbprint.
///
/// # Safety
/// `state` is the state data of a verify call that has not been closed yet.
unsafe fn check_signer(state: HANDLE) -> Result<(), SignError> {
    let missing = || SignError::Os("no signer data".to_string());
    // SAFETY: the caller guarantees `state` is live verify state.
    let provider = unsafe { WTHelperProvDataFromStateData(state) };
    if provider.is_null() {
        return Err(missing());
    }
    // SAFETY: non-null provider data owned by the open state; read only while it is open.
    let steps = unsafe {
        let data = &*provider;
        if data.padwTrustStepErrors.is_null() {
            return Err(missing());
        }
        std::slice::from_raw_parts(data.padwTrustStepErrors, data.cdwTrustStepErrors as usize)
    };
    // A file whose digest or signature failed never gets here, but the forgiven root must not
    // hide such a failure even if Windows ranked it lower.
    let early = [
        TRUSTERROR_STEP_FINAL_WVTINIT,
        TRUSTERROR_STEP_FINAL_INITPROV,
        TRUSTERROR_STEP_FINAL_OBJPROV,
        TRUSTERROR_STEP_FINAL_SIGPROV,
    ];
    if early
        .iter()
        .any(|&step| steps.get(step as usize).is_none_or(|&error| error != 0))
    {
        return Err(SignError::BadSignature);
    }
    // SAFETY: `provider` is live; signer 0 is the primary signature, no countersigner.
    let signer = unsafe { WTHelperGetProvSignerFromChain(provider, 0, false, 0) };
    if signer.is_null() {
        return Err(missing());
    }
    // SAFETY: non-null signer owned by the open state; its chain array has `csCertChain` items.
    let leaf = unsafe {
        let signer = &*signer;
        if signer.csCertChain == 0 || signer.pasCertChain.is_null() {
            return Err(missing());
        }
        (*signer.pasCertChain).pCert
    };
    if leaf.is_null() {
        return Err(missing());
    }
    // SAFETY: `leaf` is the signer certificate context, alive while the state is open.
    let thumb = unsafe { thumbprint(leaf) }.map_err(SignError::Os)?;
    if thumb == PLITH_THUMBPRINT {
        Ok(())
    } else {
        Err(SignError::WrongSigner(thumb))
    }
}

/// The SHA-1 thumbprint of a certificate, upper-case hex.
///
/// # Safety
/// `cert` points to a valid certificate context.
unsafe fn thumbprint(cert: *const CERT_CONTEXT) -> Result<String, String> {
    let mut hash = [0u8; 20];
    let mut size = 20u32;
    // SAFETY: the caller guarantees `cert`; the buffer holds `size` bytes.
    unsafe {
        CertGetCertificateContextProperty(
            cert,
            CERT_SHA1_HASH_PROP_ID,
            Some(hash.as_mut_ptr().cast()),
            &raw mut size,
        )
    }
    .map_err(|error| format!("thumbprint: {error}"))?;
    let mut text = String::with_capacity(40);
    for byte in hash.iter().take(size as usize) {
        let _ = write!(text, "{byte:02X}");
    }
    Ok(text)
}

/// `size_of::<T>()` as the `u32` a `cbStruct` field takes.
fn size_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("struct size fits u32")
}
