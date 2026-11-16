use super::*;
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, E_FAIL, E_OUTOFMEMORY};
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
};

#[test]
fn device_loss_codes_are_recognised() {
    for code in [
        DXGI_ERROR_DEVICE_REMOVED,
        DXGI_ERROR_DEVICE_RESET,
        DXGI_ERROR_DEVICE_HUNG,
        D2DERR_RECREATE_TARGET,
    ] {
        assert!(is_device_lost(code), "{code:?}");
    }
    for code in [E_FAIL, E_OUTOFMEMORY] {
        assert!(!is_device_lost(code), "{code:?}");
    }
}
