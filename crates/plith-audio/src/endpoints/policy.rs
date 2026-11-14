//! The undocumented `IPolicyConfig`, the only way to change the default render device (every
//! app that offers the feature uses it). Declared by hand, only as far as `SetDefaultEndpoint`.

// The `interface` expansion transmutes pointer references; that code is not ours.
#![allow(clippy::transmute_ptr_to_ptr)]

use windows::Win32::Media::Audio::ERole;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR};
use windows_core::interface;

use super::write::DEFAULT_ROLES;

/// `CPolicyConfigClient`.
const POLICY_CONFIG_CLSID: GUID = GUID::from_u128(0x870a_f99c_171d_4f9e_af0d_e63d_f40c_2bc9);

/// `IPolicyConfig`, vtable after `IUnknown`: TEN reserved slots, then `SetDefaultEndpoint`.
///
/// The count is measured, not guessed: the WPF app declared nine and the call landed one slot
/// early on `SetPropertyValue` (0x800706F4, a role number read as a `PROPERTYKEY` pointer). The
/// order is `GetMixFormat`, `GetDeviceFormat`, `ResetDeviceFormat`, `SetDeviceFormat`,
/// `GetProcessingPeriod`, `SetProcessingPeriod`, `GetShareMode`, `SetShareMode`,
/// `GetPropertyValue`, `SetPropertyValue`. The reserved slots are never called, only counted.
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn reserved_1(&self) -> HRESULT;
    fn reserved_2(&self) -> HRESULT;
    fn reserved_3(&self) -> HRESULT;
    fn reserved_4(&self) -> HRESULT;
    fn reserved_5(&self) -> HRESULT;
    fn reserved_6(&self) -> HRESULT;
    fn reserved_7(&self) -> HRESULT;
    fn reserved_8(&self) -> HRESULT;
    fn reserved_9(&self) -> HRESULT;
    fn reserved_10(&self) -> HRESULT;
    fn set_default_endpoint(&self, device_id: PCWSTR, role: ERole) -> HRESULT;
}

/// Makes `id` the default render endpoint for the Console and Multimedia roles. `true` only
/// when both writes succeeded. Runs on the service thread.
pub(super) fn set_default_endpoint(id: &str) -> bool {
    let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the class and interface ids are the observed pair for the policy client; `wide`
    // is NUL terminated and outlives every call.
    let write = || -> windows::core::Result<()> {
        unsafe {
            let config: IPolicyConfig = CoCreateInstance(&POLICY_CONFIG_CLSID, None, CLSCTX_ALL)?;
            DEFAULT_ROLES.iter().try_for_each(|role| {
                config
                    .set_default_endpoint(PCWSTR(wide.as_ptr()), *role)
                    .ok()
            })
        }
    };
    write().is_ok()
}
