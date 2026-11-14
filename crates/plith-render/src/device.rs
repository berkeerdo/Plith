//! The GPU device stack: D3D11, DXGI, Direct2D and DirectComposition.

use windows::Win32::Foundation::{E_POINTER, HMODULE};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
    ID2D1DeviceContext, ID2D1Factory1,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{DCompositionCreateDevice, IDCompositionDevice};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice, IDXGIFactory2};
use windows::core::{Error, Interface, Result};

/// Every device object the renderer needs, created together.
pub struct Gpu {
    d3d: ID3D11Device,
    dxgi_device: IDXGIDevice,
    dxgi_factory: IDXGIFactory2,
    d2d_factory: ID2D1Factory1,
    d2d_context: ID2D1DeviceContext,
    dcomp: IDCompositionDevice,
    warp: bool,
}

fn create_d3d(driver: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
    let mut device: Option<ID3D11Device> = None;
    // SAFETY: the out pointer refers to a live local; the other pointers are null or absent.
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&raw mut device),
            None,
            None,
        )?;
    }
    device.ok_or_else(|| Error::from(E_POINTER))
}

impl Gpu {
    /// Creates the device stack on the hardware adapter, falling back to WARP on failure
    /// or when `prefer_warp` is set.
    ///
    /// # Errors
    /// Returns the first failing Windows call, including a failing WARP device.
    pub fn new(prefer_warp: bool) -> Result<Self> {
        let (d3d, warp) = if prefer_warp {
            (create_d3d(D3D_DRIVER_TYPE_WARP)?, true)
        } else {
            match create_d3d(D3D_DRIVER_TYPE_HARDWARE) {
                Ok(device) => (device, false),
                Err(_) => (create_d3d(D3D_DRIVER_TYPE_WARP)?, true),
            }
        };
        let dxgi_device: IDXGIDevice = d3d.cast()?;
        // SAFETY: plain COM calls on live interfaces owned by this function.
        unsafe {
            let dxgi_factory: IDXGIFactory2 = dxgi_device.GetAdapter()?.GetParent()?;
            let d2d_factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d_device = d2d_factory.CreateDevice(&dxgi_device)?;
            let d2d_context = d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            Ok(Self {
                d3d,
                dxgi_device,
                dxgi_factory,
                d2d_factory,
                d2d_context,
                dcomp,
                warp,
            })
        }
    }

    /// True when the device runs on the WARP software adapter.
    #[must_use]
    pub fn is_warp(&self) -> bool {
        self.warp
    }

    /// The Direct2D device context every draw goes through.
    #[must_use]
    pub fn context(&self) -> &ID2D1DeviceContext {
        &self.d2d_context
    }

    /// The Direct3D 11 device.
    #[must_use]
    pub fn d3d(&self) -> &ID3D11Device {
        &self.d3d
    }

    /// The DXGI view of the Direct3D device.
    #[must_use]
    pub fn dxgi_device(&self) -> &IDXGIDevice {
        &self.dxgi_device
    }

    /// The DXGI factory of the device's adapter.
    #[must_use]
    pub fn dxgi_factory(&self) -> &IDXGIFactory2 {
        &self.dxgi_factory
    }

    /// The single-threaded Direct2D factory.
    #[must_use]
    pub fn d2d_factory(&self) -> &ID2D1Factory1 {
        &self.d2d_factory
    }

    /// The DirectComposition device.
    #[must_use]
    pub fn dcomp(&self) -> &IDCompositionDevice {
        &self.dcomp
    }
}
