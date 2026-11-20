//! The settings window's swap chain: a plain rectangle the size of the client area, shown
//! through DirectComposition. The floor is painted opaque, so there is no island shape.

use crate::device::Gpu;
use crate::target::bind_back_buffer;
use windows::Win32::Foundation::{E_INVALIDARG, HWND};
use windows::Win32::Graphics::Direct2D::ID2D1Image;
use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGISwapChain1,
};
use windows::core::{Error, Result};

/// A flip-model swap chain on the settings window.
pub(crate) struct SheetSurface {
    swap_chain: IDXGISwapChain1,
    // Kept alive: dropping the target or the visual detaches the content from the window.
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

fn check_size(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(Error::from(E_INVALIDARG));
    }
    Ok(())
}

impl SheetSurface {
    /// A `width` by `height` pixel swap chain attached to `hwnd`, its back buffer bound as the
    /// target of `gpu.context()` at `96 * scale` DPI.
    ///
    /// # Errors
    /// Returns the first failing Windows call, or `E_INVALIDARG` for a zero size.
    pub(crate) fn new(
        gpu: &Gpu,
        hwnd: HWND,
        (width, height): (u32, u32),
        scale: f64,
    ) -> Result<Self> {
        check_size(width, height)?;
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            ..Default::default()
        };
        // SAFETY: plain COM calls on live interfaces; `desc` outlives the call that reads it.
        let (swap_chain, target, visual) = unsafe {
            let swap_chain = gpu.dxgi_factory().CreateSwapChainForComposition(
                gpu.d3d(),
                &raw const desc,
                None,
            )?;
            let dcomp = gpu.dcomp();
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap_chain)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            (swap_chain, target, visual)
        };
        bind_back_buffer(gpu, &swap_chain, scale)?;
        Ok(Self {
            swap_chain,
            _target: target,
            _visual: visual,
        })
    }

    /// Resizes the buffers to `width` by `height` pixels and rebinds the back buffer at
    /// `96 * scale` DPI.
    ///
    /// # Errors
    /// Returns the failing resize or bind call, or `E_INVALIDARG` for a zero size.
    pub(crate) fn resize(&self, gpu: &Gpu, (width, height): (u32, u32), scale: f64) -> Result<()> {
        check_size(width, height)?;
        Self::release(gpu);
        // SAFETY: the context no longer holds the back buffer; 0 and UNKNOWN keep the buffer
        // count and the format.
        unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                width,
                height,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        bind_back_buffer(gpu, &self.swap_chain, scale)
    }

    /// Drops the context's reference to the back buffer.
    pub(crate) fn release(gpu: &Gpu) {
        // SAFETY: clearing the target of a live context is always valid.
        unsafe { gpu.context().SetTarget(None::<&ID2D1Image>) };
    }

    /// Presents the back buffer on the next vblank.
    ///
    /// # Errors
    /// Returns the `Present` HRESULT when it is a failure (for example device removed).
    pub(crate) fn present(&self) -> Result<()> {
        // SAFETY: a live swap chain; the flags value 0 is valid.
        unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)).ok() }
    }
}
