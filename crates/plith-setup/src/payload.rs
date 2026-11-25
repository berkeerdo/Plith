//! The files the installer carries: the app, its UIAccess helper and the public certificate.

/// The bytes of everything the installer installs.
#[derive(Debug, Clone, Copy)]
pub struct Payload {
    /// The signed `plith.exe`.
    pub plith: &'static [u8],
    /// The signed `plith-osd.exe`.
    pub helper: &'static [u8],
    /// The public part of the signing certificate (DER).
    pub cert: &'static [u8],
}

impl Payload {
    /// True when both executables are present: a dev build embeds empty stand-ins.
    #[must_use]
    pub fn is_release(&self) -> bool {
        !self.plith.is_empty() && !self.helper.is_empty()
    }
}

/// The payload compiled into this exe.
#[must_use]
pub fn embedded() -> Payload {
    Payload {
        plith: include_bytes!(concat!(env!("OUT_DIR"), "/plith.exe")),
        helper: include_bytes!(concat!(env!("OUT_DIR"), "/plith-osd.exe")),
        cert: plith_host::trust::PLITH_CERT,
    }
}
