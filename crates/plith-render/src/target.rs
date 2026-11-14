//! The DirectComposition swap chain bound to the notch window.

use plith_core::layout::SurfaceLayout;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
};
use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice1, IDXGISurface, IDXGISwapChain1,
};
use windows::core::{Interface, Result};

use crate::device::Gpu;

/// A flip-model swap chain shown through DirectComposition on one window.
pub struct CompositionTarget {
    swap_chain: IDXGISwapChain1,
    // Kept alive: dropping the target or the visual detaches the content from the window.
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

/// Wraps the swap chain back buffer as the Direct2D target of the context.
fn bind_back_buffer(gpu: &Gpu, swap_chain: &IDXGISwapChain1, scale: f64) -> Result<()> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        colorContext: std::mem::ManuallyDrop::new(None),
    };
    // SAFETY: buffer 0 exists on a swap chain with two buffers; `props` is live for the call.
    unsafe {
        let surface: IDXGISurface = swap_chain.GetBuffer(0)?;
        let bitmap = gpu
            .context()
            .CreateBitmapFromDxgiSurface(&surface, Some(&raw const props))?;
        gpu.context().SetTarget(&bitmap);
        #[allow(clippy::cast_possible_truncation)]
        let dpi = (96.0 * scale) as f32;
        gpu.context().SetDpi(dpi, dpi);
    }
    Ok(())
}

impl CompositionTarget {
    /// Creates the swap chain sized to `layout.surface`, attaches it to `hwnd` and makes the
    /// back buffer the Direct2D target of `gpu.context()` at `96 * scale` DPI.
    ///
    /// # Errors
    /// Returns the first failing Windows call.
    pub fn new(gpu: &Gpu, hwnd: HWND, layout: &SurfaceLayout) -> Result<Self> {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: u32::try_from(layout.surface.width).unwrap_or(1),
            Height: u32::try_from(layout.surface.height).unwrap_or(1),
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
            gpu.dxgi_device()
                .cast::<IDXGIDevice1>()?
                .SetMaximumFrameLatency(1)?;
            let dcomp = gpu.dcomp();
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap_chain)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            (swap_chain, target, visual)
        };
        bind_back_buffer(gpu, &swap_chain, layout.scale)?;
        Ok(Self {
            swap_chain,
            _target: target,
            _visual: visual,
        })
    }

    /// Presents the back buffer, waiting for one vertical blank.
    ///
    /// # Errors
    /// Returns the `Present` HRESULT when it is a failure (for example device removed).
    pub fn present(&mut self) -> Result<()> {
        // SAFETY: `swap_chain` is a live swap chain; the flags value 0 is valid.
        unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)).ok() }
    }
}
